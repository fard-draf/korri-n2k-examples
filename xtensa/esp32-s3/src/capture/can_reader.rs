//! Bus reader: receive, timestamp, publish. Nothing else.
//!
//! This task is the only place where a frame can be lost for good, hence three
//! rules: timestamp on the very first instruction, publish without ever
//! blocking, and format nothing inside the loop (a `defmt!` costs tens of
//! microseconds, for one frame every 600 µs).

use core::sync::atomic::Ordering;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embedded_can::{Frame, Id};
use esp_hal::{
    time::Instant,
    twai::{ErrorKind, EspTwaiError, Twai},
    Async,
};

use super::frame::{TimestampedFrame, FLAG_EXTENDED, FLAG_REMOTE, STATS};
use super::CAPTURE_DEPTH;

/// Two receptions closer than this cannot come from the wire (an extended frame
/// takes ~600 µs): we are draining a backlog from the esp-hal queue.
const BACKLOG_GAP_US: u64 = 100;

/// Systimer rather than `embassy_time`: native microseconds, a few cycles to read.
#[inline(always)]
fn now_us() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

#[embassy_executor::task]
pub async fn can_reader(
    mut can: Twai<'static, Async>,
    sender: Sender<'static, CriticalSectionRawMutex, TimestampedFrame, CAPTURE_DEPTH>,
) {
    let mut previous_us: u64 = 0;
    let mut backlog_run: u32 = 0;

    loop {
        match can.receive_async().await {
            Ok(raw) => {
                // Anything after this line adds error to the timestamp.
                let timestamp_us = now_us();

                if previous_us != 0 && timestamp_us.saturating_sub(previous_us) < BACKLOG_GAP_US {
                    backlog_run += 1;
                    STATS
                        .max_backlog_run
                        .fetch_max(backlog_run, Ordering::Relaxed);
                } else {
                    backlog_run = 0;
                }
                previous_us = timestamp_us;

                // Standard frames are kept: we capture all traffic and sort it
                // out offline.
                let (id, id_flag) = match raw.id() {
                    Id::Extended(id) => (id.as_raw(), FLAG_EXTENDED),
                    Id::Standard(id) => (id.as_raw() as u32, 0),
                };

                let payload = raw.data();
                let len = payload.len().min(8);
                let mut data = [0u8; 8];
                data[..len].copy_from_slice(&payload[..len]);

                let mut flags = id_flag;
                if raw.is_remote_frame() {
                    flags |= FLAG_REMOTE;
                }

                let frame = TimestampedFrame {
                    timestamp_us,
                    id,
                    data,
                    len: len as u8,
                    flags,
                };

                STATS.frames_rx.fetch_add(1, Ordering::Relaxed);

                // `try_send`, never `send().await`: a full channel costs one
                // frame, waiting would cost dozens.
                if sender.try_send(frame).is_err() {
                    STATS.channel_drops.fetch_add(1, Ordering::Relaxed);
                }

                STATS
                    .max_channel_depth
                    .fetch_max(sender.len() as u32, Ordering::Relaxed);
            }
            // Frames lost before they even reached the reader.
            Err(EspTwaiError::EmbeddedHAL(ErrorKind::Overrun)) => {
                STATS.hw_overruns.fetch_add(1, Ordering::Relaxed);
            }
            // Impossible in listen-only mode; counted just in case. No recovery:
            // it would mean rebuilding the driver.
            Err(EspTwaiError::BusOff) => {
                STATS.bus_off.fetch_add(1, Ordering::Relaxed);
            }
            Err(_) => {
                STATS.soft_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
