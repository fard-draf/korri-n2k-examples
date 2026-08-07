#![no_std]
#![no_main]

//! Passive Fast Packet receiver.
//!
//! Listens on the N2K bus, feeds every Fast Packet fragment to
//! `korri_n2k`'s `FastPacketAssembler` and logs each reassembled payload over
//! RTT. No address claim: this node only listens, it never talks.

use defmt_rtt as _;
use esp_backtrace as _;

use embassy_time::Instant;
use esp32_s3::{
    conf::DIAG_PERIOD_MS,
    ports::EspCanBus,
    starter::{init_bsp, init_can},
};
use korri_n2k::protocol::transport::{
    fast_packet::{
        FAST_PACKET_PGNS_ALL,
        assembler::{FastPacketAssembler, ProcessResult},
    },
    traits::can_bus::CanBus,
};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut bus = EspCanBus::new(init_can(init_bsp()));

    // Every Fast Packet PGN canboat declares, not only those in the manifest.
    // A listener forwards what it cannot decode, so it wants the full table.
    let mut assembler = FastPacketAssembler::with_pgns(FAST_PACKET_PGNS_ALL);
    let mut last_diag = Instant::now();

    defmt::info!(
        "Fast Packet receiver up: {=usize} Fast Packet PGNs known",
        FAST_PACKET_PGNS_ALL.len()
    );

    loop {
        let frame = match bus.recv().await {
            Ok(frame) => frame,
            Err(err) => {
                defmt::error!("bus recv failed: {}", err);
                continue;
            }
        };

        let pgn = frame.id.pgn();
        let source = frame.id.source_address();

        // Branch on `handles`, never on the decode result: the first fragment of
        // a Fast Packet decodes fine and returns wrong values.
        if assembler.handles(pgn) {
            let now_ms = Instant::now().as_millis() as u32;
            match assembler.process_frame(now_ms, pgn, source, &frame.data) {
                ProcessResult::MessageComplete(msg) => defmt::info!(
                    "PGN {=u32} from {=u8}: {=usize} bytes {=[u8]:02x}",
                    pgn,
                    source,
                    msg.len,
                    &msg.payload[..msg.len]
                ),
                ProcessResult::FragmentConsumed => {
                    defmt::trace!("PGN {=u32} from {=u8}: fragment consumed", pgn, source)
                }
                ProcessResult::Ignored => {
                    defmt::debug!("PGN {=u32} from {=u8}: frame ignored", pgn, source)
                }
            }
        } else {
            defmt::debug!(
                "PGN {=u32} from {=u8}: single frame, {=usize} bytes",
                pgn,
                source,
                frame.len
            );
        }

        // Frames arrive continuously on a live bus, so the loop itself paces the
        // diagnostics: no separate timer task.
        if last_diag.elapsed().as_millis() >= DIAG_PERIOD_MS {
            last_diag = Instant::now();
            defmt::debug!(
                "assembler: lost_fragments={=u32} expired={=u32} pool_exhausted={=u32} rejected={=u32} unknown_pgn={=u32}",
                assembler.lost_fragments(),
                assembler.expired_sessions(),
                assembler.pool_exhausted(),
                assembler.rejected_frames(),
                assembler.unknown_pgn()
            );
        }
    }
}
