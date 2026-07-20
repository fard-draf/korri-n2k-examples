//! Binary format of the capture stream: fixed 24-byte records, little-endian.
//!
//! Every record carries a sequence number, so the decoder can spot losses that
//! happened on the USB link — which the target's own counters cannot see.

use super::frame::{StatsSnapshot, TimestampedFrame};

pub const RECORD_SIZE: usize = 24;

/// Session marker, also the resync point for a host that attaches mid-capture.
pub const MAGIC: [u8; 8] = *b"KN2KCAP\x01";

pub const RECORD_FRAME: u8 = 0x01;
pub const RECORD_STATS: u8 = 0x02;

/// `MAGIC` | bus bitrate | record size.
pub fn encode_header(bitrate: u32) -> [u8; RECORD_SIZE] {
    let mut out = [0u8; RECORD_SIZE];
    out[0..8].copy_from_slice(&MAGIC);
    out[8..12].copy_from_slice(&bitrate.to_le_bytes());
    out[12..16].copy_from_slice(&(RECORD_SIZE as u32).to_le_bytes());
    out
}

/// | 0 type | 1 len+flags | 2 sequence | 4 id | 8 timestamp_us | 16 data |
pub fn encode_frame(frame: &TimestampedFrame, seq: u16) -> [u8; RECORD_SIZE] {
    let mut out = [0u8; RECORD_SIZE];
    out[0] = RECORD_FRAME;
    out[1] = (frame.len & 0x0F) | (frame.flags << 4);
    out[2..4].copy_from_slice(&seq.to_le_bytes());
    out[4..8].copy_from_slice(&frame.id.to_le_bytes());
    out[8..16].copy_from_slice(&frame.timestamp_us.to_le_bytes());
    out[16..24].copy_from_slice(&frame.data);
    out
}

/// Counter snapshot. No timestamp of its own: the decoder dates it from the
/// preceding frame. Counters that stay small are saturated into shorter
/// integers to fit in 24 bytes.
pub fn encode_stats(stats: &StatsSnapshot, seq: u16) -> [u8; RECORD_SIZE] {
    let mut out = [0u8; RECORD_SIZE];
    out[0] = RECORD_STATS;
    out[1] = 0;
    out[2..4].copy_from_slice(&seq.to_le_bytes());
    out[4..8].copy_from_slice(&stats.frames_rx.to_le_bytes());
    out[8..12].copy_from_slice(&stats.channel_drops.to_le_bytes());
    out[12..16].copy_from_slice(&stats.hw_overruns.to_le_bytes());
    out[16..18].copy_from_slice(&saturate_u16(stats.soft_errors).to_le_bytes());
    out[18..20].copy_from_slice(&saturate_u16(stats.sink_drops).to_le_bytes());
    out[20..22].copy_from_slice(&saturate_u16(stats.max_channel_depth).to_le_bytes());
    out[22] = saturate_u8(stats.max_backlog_run);
    out[23] = saturate_u8(stats.bus_off);
    out
}

fn saturate_u16(value: u32) -> u16 {
    value.min(u16::MAX as u32) as u16
}

fn saturate_u8(value: u32) -> u8 {
    value.min(u8::MAX as u32) as u8
}
