//! Timestamped frame and integrity counters.

use core::sync::atomic::{AtomicU32, Ordering};

/// Extended frame (29-bit id). Absent means standard frame (11-bit id).
pub const FLAG_EXTENDED: u8 = 0b0000_0001;
/// Remote Transmission Request: the frame carries no data.
pub const FLAG_REMOTE: u8 = 0b0000_0010;

/// A CAN frame timestamped on reception.
///
/// The timestamp marks the moment the frame leaves the esp-hal queue, not its
/// arrival on the wire: a few microseconds off while the reader keeps up, more
/// during a burst. `max_backlog_run` measures that drift.
#[derive(Clone, Copy, Debug)]
pub struct TimestampedFrame {
    /// Microseconds since boot.
    pub timestamp_us: u64,
    /// Right-aligned CAN identifier (29 or 11 bits, see `flags`).
    pub id: u32,
    pub data: [u8; 8],
    pub len: u8,
    pub flags: u8,
}

/// Counters fed by the reader and the sink, read once per second by `usb_sink`.
#[derive(Debug)]
pub struct CaptureStats {
    /// Frames received and timestamped.
    pub frames_rx: AtomicU32,
    /// Frames dropped because the channel was full.
    pub channel_drops: AtomicU32,
    /// TWAI hardware FIFO overruns.
    pub hw_overruns: AtomicU32,
    /// Bus-off events (should never happen in listen-only mode).
    pub bus_off: AtomicU32,
    /// Transient errors (bad DLC, bus errors).
    pub soft_errors: AtomicU32,
    /// Frames lost on USB write (host missing or too slow).
    pub sink_drops: AtomicU32,
    /// Highest channel occupancy reached.
    pub max_channel_depth: AtomicU32,
    /// Longest run of frames drained from a backlog. Past 32, esp-hal starts
    /// dropping frames without reporting it.
    pub max_backlog_run: AtomicU32,
}

impl CaptureStats {
    pub const fn new() -> Self {
        Self {
            frames_rx: AtomicU32::new(0),
            channel_drops: AtomicU32::new(0),
            hw_overruns: AtomicU32::new(0),
            bus_off: AtomicU32::new(0),
            soft_errors: AtomicU32::new(0),
            sink_drops: AtomicU32::new(0),
            max_channel_depth: AtomicU32::new(0),
            max_backlog_run: AtomicU32::new(0),
        }
    }

    /// Fields are not read atomically with respect to each other: good enough
    /// for diagnostics, and no critical section on the hot path.
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            frames_rx: self.frames_rx.load(Ordering::Relaxed),
            channel_drops: self.channel_drops.load(Ordering::Relaxed),
            hw_overruns: self.hw_overruns.load(Ordering::Relaxed),
            bus_off: self.bus_off.load(Ordering::Relaxed),
            soft_errors: self.soft_errors.load(Ordering::Relaxed),
            sink_drops: self.sink_drops.load(Ordering::Relaxed),
            max_channel_depth: self.max_channel_depth.load(Ordering::Relaxed),
            max_backlog_run: self.max_backlog_run.load(Ordering::Relaxed),
        }
    }
}

impl Default for CaptureStats {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StatsSnapshot {
    pub frames_rx: u32,
    pub channel_drops: u32,
    pub hw_overruns: u32,
    pub bus_off: u32,
    pub soft_errors: u32,
    pub sink_drops: u32,
    pub max_channel_depth: u32,
    pub max_backlog_run: u32,
}

impl StatsSnapshot {
    pub fn is_lossless(&self) -> bool {
        self.channel_drops == 0 && self.hw_overruns == 0 && self.sink_drops == 0
    }
}

pub static STATS: CaptureStats = CaptureStats::new();
