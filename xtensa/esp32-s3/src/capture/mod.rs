//! Complete, timestamped capture of an NMEA2000 bus.
//!
//! ```text
//! bus ─► TWAI FIFO (4) ─► ISR ─► esp-hal queue (32) ─► can_reader ─► CAPTURE_CHANNEL (1024) ─► usb_sink ─► USB
//!        └─ 2.4 ms ─┘                   └─ 19 ms ─┘                        └─ 610 ms ─┘
//! ```
//!
//! Each buffer only covers the delay of the stage right after it: interrupt
//! latency, then task latency, then USB host slowness.
//!
//! The node is a passive listener: it never transmits, never claims an address
//! and never acknowledges. Nothing can block reception, and the observed bus is
//! left untouched.

pub mod can_reader;
pub mod frame;
pub mod usb_sink;
pub mod wire;

/// 24 KB of SRAM, about 610 ms of headroom at full load (~1670 frames/s).
/// Enough to absorb a USB host stall without ever stalling the reader.
pub const CAPTURE_DEPTH: usize = 1024;

pub use can_reader::can_reader;
pub use frame::{CaptureStats, StatsSnapshot, TimestampedFrame, STATS};
pub use usb_sink::usb_sink;
