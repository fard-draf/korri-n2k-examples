//! Full NMEA 2000 node on Linux SocketCAN.
//!
//! Same shape as the boards: one identity from `shared-core`, the same PGN
//! generators, the same publishing loop. Only the driver and the runtime differ.
//!
//! ```bash
//! cargo run -- can0 3
//! ```

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use korri_n2k::protocol::management::address_manager::AddressManager;
use korri_n2k::protocol::management::address_supervisor::AddressService;
use korri_n2k::protocol::management::iso_name::IsoName;
use korri_n2k::protocol::transport::can_frame::CanFrame;
use korri_n2k::protocol::transport::can_id::CanId;
use korri_n2k::protocol::transport::fast_packet::FAST_PACKET_PGNS_ALL;
use korri_n2k::protocol::transport::fast_packet::assembler::{FastPacketAssembler, ProcessResult};
use korri_n2k::protocol::transport::traits::can_bus::CanBus;
use korri_n2k::protocol::transport::traits::korri_timer::{Clock, TokioTimer};
use shared_core::instances::{
    IDENTITY_1, IDENTITY_2, IDENTITY_3, IDENTITY_4, IDENTITY_5, IsoIdentity,
};
use shared_core::publisher::publish;
use shared_core::samples::{Depth, Position, Speed};
use socketcan::tokio::CanSocket;
use socketcan::{CanDataFrame, EmbeddedFrame, ExtendedId, Id};

/// SocketCAN driver.
///
/// Two rules the `CanBus` contract asks for, and how they are met here.
///
/// `recv` is cancelled often, so it must be safe to drop. It is: the frame
/// stays in the kernel socket buffer until the read actually succeeds.
///
/// Any `Err` stops the runner for good, so transient conditions are absorbed
/// rather than returned. A full transmit queue is retried, not reported.
struct SocketCanBus {
    socket: CanSocket,
}

impl SocketCanBus {
    fn open(interface: &str) -> Result<Self> {
        let socket =
            CanSocket::open(interface).with_context(|| format!("cannot open {interface}"))?;
        Ok(Self { socket })
    }
}

impl CanBus for SocketCanBus {
    type Error = std::io::Error;

    async fn send(&mut self, frame: &CanFrame) -> Result<(), Self::Error> {
        let id = ExtendedId::new(frame.id.0).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a 29-bit identifier")
        })?;
        let data = CanDataFrame::new(id, &frame.data[..frame.len]).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "payload over 8 bytes")
        })?;

        loop {
            match self.socket.write_frame(data.into()).await {
                Ok(()) => return Ok(()),
                // The transmit queue is full: back off instead of killing the
                // runner. Only a broken socket is worth reporting.
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn recv(&mut self) -> Result<CanFrame, Self::Error> {
        loop {
            let frame = self.socket.read_frame().await?;

            // Standard identifiers and error frames are not NMEA 2000.
            let Id::Extended(id) = frame.id() else {
                continue;
            };

            let payload = frame.data();
            let mut data = [0u8; 8];
            data[..payload.len()].copy_from_slice(payload);

            return Ok(CanFrame {
                id: CanId(id.as_raw()),
                data,
                len: payload.len(),
            });
        }
    }
}

/// Build the NAME from the shared identity, exactly as `app.rs` does on a board.
fn iso_name(identity: &IsoIdentity) -> IsoName {
    IsoName::builder()
        .unique_number(identity.unique_number)
        .manufacturer_code(identity.manufacturer_code)
        .device_function(identity.device_function)
        .device_class(identity.device_class)
        .device_instance(identity.device_instance)
        .system_instance(identity.system_instance)
        .industry_group(identity.industry_group)
        .arbitrary_address_capable(identity.is_arbitrary_address_capable())
        .build()
}

fn identity(number: &str) -> Result<&'static IsoIdentity> {
    match number {
        "1" => Ok(&IDENTITY_1),
        "2" => Ok(&IDENTITY_2),
        "3" => Ok(&IDENTITY_3),
        "4" => Ok(&IDENTITY_4),
        "5" => Ok(&IDENTITY_5),
        _ => bail!("identity must be 1, 2, 3, 4, or 5; received {number}"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let interface = arguments.next().unwrap_or_else(|| "can0".to_string());
    let identity_number = arguments.next().unwrap_or_else(|| "3".to_string());
    let identity = identity(&identity_number)?;

    println!("opening {interface} with shared-core IDENTITY_{identity_number}");

    let bus = SocketCanBus::open(&interface)?;
    let name = iso_name(identity);
    println!(
        "ISO NAME: 0x{:016X}, strategy {:?}",
        name.raw(),
        identity.strategy
    );

    // Synchronous. It only fails when the NAME contradicts the strategy, and the
    // identity derives the AAC bit from that strategy, so it cannot.
    let manager = AddressManager::new(bus, TokioTimer::new(), name, identity.strategy)
        .map_err(|fault| anyhow::anyhow!("{fault}"))
        .context("the NAME must match its address claim strategy")?;

    // 4 queued commands, 64 buffered incoming frames. A full frame channel drops
    // rather than queues: address management is never delayed by a slow reader.
    let parts = AddressService::new(manager, 4, 64).into_parts();
    let handle = parts.handle.expect("a command channel was requested");
    let mut frames = parts.frames.expect("a frame channel was requested");

    // The claim starts here. `drive` returns only on a bus error, and that error
    // is terminal: the node keeps no address afterwards.
    tokio::spawn(async move {
        if let Err(error) = parts.runner.drive().await {
            eprintln!("address management stopped: {error:?}");
        }
    });

    // The same generators the boards publish, one task each.
    let handle = Arc::new(handle);
    tokio::spawn(publish(handle.clone(), Position::new()));
    tokio::spawn(publish(handle.clone(), Depth::new()));
    tokio::spawn(publish(handle, Speed::new()));

    // Every Fast Packet PGN canboat declares, not only those in the manifest.
    // A listener forwards what it cannot decode, so it wants the full table.
    let mut assembler = FastPacketAssembler::with_pgns(FAST_PACKET_PGNS_ALL);
    let clock = TokioTimer::new();

    // `None` means the runner stopped and dropped the channel.
    while let Some(frame) = frames.recv().await {
        let pgn = frame.id.pgn();
        let source = frame.id.source_address();

        // Branch on `handles`, never on the decode result: the first fragment of
        // a Fast Packet decodes fine and returns wrong values.
        if !assembler.handles(pgn) {
            println!(
                "PGN {pgn:>6} from {source:>3}: {:02X?}",
                &frame.data[..frame.len]
            );
            continue;
        }

        let now_ms = clock.now_ms() as u32;
        if let ProcessResult::MessageComplete(message) =
            assembler.process_frame(now_ms, pgn, source, &frame.data)
        {
            println!(
                "PGN {pgn:>6} from {source:>3}: {} bytes {:02X?}",
                message.len,
                &message.payload[..message.len]
            );
        }
    }

    println!("runner gone, nothing left to read");
    Ok(())
}
