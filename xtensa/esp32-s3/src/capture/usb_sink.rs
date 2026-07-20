//! Serialises the channel to the native USB Serial/JTAG port.
//!
//! Writes are batched: USB works in 64-byte packets, so one write per record
//! would collapse throughput. This task also emits the counter snapshots, since
//! it owns the port.

use core::sync::atomic::Ordering;

use defmt::{info, warn};
use embassy_futures::select::{select, Either};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::{Duration, Instant, Timer};
use embedded_io_async::Write;
use esp_hal::{usb_serial_jtag::UsbSerialJtagTx, Async};

use super::frame::{TimestampedFrame, STATS};
use super::wire::{encode_frame, encode_header, encode_stats, RECORD_SIZE};
use super::CAPTURE_DEPTH;

/// Frames per batch, plus room for one header and one stats record.
const BATCH_FRAMES: usize = 21;
const FRAMES_BYTES: usize = RECORD_SIZE * BATCH_FRAMES;
const BATCH_BYTES: usize = RECORD_SIZE * (BATCH_FRAMES + 2);

const STATS_PERIOD: Duration = Duration::from_secs(1);

#[embassy_executor::task]
pub async fn usb_sink(
    mut usb: UsbSerialJtagTx<'static, Async>,
    receiver: Receiver<'static, CriticalSectionRawMutex, TimestampedFrame, CAPTURE_DEPTH>,
    bitrate: u32,
) {
    let header = encode_header(bitrate);
    let mut buffer = [0u8; BATCH_BYTES];
    let mut seq: u16 = 0;

    // Already expired, so the header and a first snapshot go out immediately.
    let mut next_stats = Instant::now();

    loop {
        let mut filled = 0usize;

        // Block on the first frame, but still honour the stats deadline if the
        // bus goes quiet.
        match select(receiver.receive(), Timer::at(next_stats)).await {
            Either::First(frame) => {
                buffer[..RECORD_SIZE].copy_from_slice(&encode_frame(&frame, next_seq(&mut seq)));
                filled = RECORD_SIZE;

                // Then take whatever is already queued: low latency when idle,
                // full batches under load.
                while filled < FRAMES_BYTES {
                    match receiver.try_receive() {
                        Ok(frame) => {
                            buffer[filled..filled + RECORD_SIZE]
                                .copy_from_slice(&encode_frame(&frame, next_seq(&mut seq)));
                            filled += RECORD_SIZE;
                        }
                        Err(_) => break,
                    }
                }
            }
            Either::Second(_) => {}
        }

        if Instant::now() >= next_stats {
            // The header is re-sent with every snapshot: the host attaches after
            // the target, so a single header would be gone before anyone reads.
            buffer[filled..filled + RECORD_SIZE].copy_from_slice(&header);
            filled += RECORD_SIZE;

            let snapshot = STATS.snapshot();
            buffer[filled..filled + RECORD_SIZE]
                .copy_from_slice(&encode_stats(&snapshot, next_seq(&mut seq)));
            filled += RECORD_SIZE;

            // From now, not `+= period`: otherwise a long USB stall catches up
            // with a burst of snapshots.
            next_stats = Instant::now() + STATS_PERIOD;

            if snapshot.is_lossless() {
                info!(
                    "capture ok: {} frames, peak depth {}, peak burst {}",
                    snapshot.frames_rx, snapshot.max_channel_depth, snapshot.max_backlog_run
                );
            } else {
                warn!(
                    "LOSS: channel={} hw_overrun={} usb={} (rx={}, peak burst {})",
                    snapshot.channel_drops,
                    snapshot.hw_overruns,
                    snapshot.sink_drops,
                    snapshot.frames_rx,
                    snapshot.max_backlog_run
                );
            }
        }

        if filled > 0 {
            write_batch(&mut usb, &buffer[..filled]).await;
        }
    }
}

fn next_seq(seq: &mut u16) -> u16 {
    let current = *seq;
    *seq = seq.wrapping_add(1);
    current
}

/// Writes a batch, with no timeout.
///
/// The channel decides what gets dropped, not the sink. Giving up on a write
/// would throw away a batch while the channel may still have room, and a
/// partial write already on the wire would desynchronise the stream. If the
/// host stops reading, the channel fills up and `can_reader` counts the losses
/// where they belong.
async fn write_batch(usb: &mut UsbSerialJtagTx<'static, Async>, bytes: &[u8]) {
    if usb.write_all(bytes).await.is_err() {
        let records = (bytes.len() / RECORD_SIZE) as u32;
        STATS.sink_drops.fetch_add(records, Ordering::Relaxed);
        return;
    }

    // Without a flush, a partial packet can sit there until the next write —
    // annoying when the bus goes quiet.
    let _ = usb.flush().await;
}
