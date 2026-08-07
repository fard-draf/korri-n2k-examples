//! Actisense framing, both directions.
//!
//! What it is for: a receiver that speaks Actisense instead of raw CAN. A board
//! reads the bus, calls [`format_actisense`] on every frame and pushes the line
//! out over USB or UART, where a plotter or SignalK reads it. [`parse_ngt1_message`]
//! goes the other way, turning what SignalK sends back into a [`CanFrame`].
//!
//! No example uses it today. The `sniffer` binary ships its own binary wire
//! format, and `receiver` logs over RTT. This module is kept because an
//! Actisense gateway is the obvious next node to build.
//!
//! Two formats, not one. [`format_actisense`] writes the plain text log line.
//! [`parse_ngt1_message`] reads the binary NGT-1 message. They do not round-trip.

use korri_n2k::protocol::transport::{can_frame::CanFrame, can_id::CanId};

/// Writes `value` as two hex digits at `pos`.
fn u8_to_hex(value: u8, buffer: &mut [u8], pos: usize) {
    const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";
    buffer[pos] = HEX_CHARS[(value >> 4) as usize];
    buffer[pos + 1] = HEX_CHARS[(value & 0x0F) as usize];
}

/// Writes `value` as eight hex digits at `pos`.
fn u32_to_hex(value: u32, buffer: &mut [u8], pos: usize) {
    const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";
    for i in 0..8 {
        let shift = (7 - i) * 4;
        let nibble = ((value >> shift) & 0x0F) as usize;
        buffer[pos + i] = HEX_CHARS[nibble];
    }
}

/// Writes `value` as two decimal digits at `pos`. Wraps silently over 99.
fn u8_to_dec(value: u8, buffer: &mut [u8], pos: usize) {
    buffer[pos] = b'0' + (value / 10);
    buffer[pos + 1] = b'0' + (value % 10);
}

/// Writes `value` as three decimal digits at `pos`. Wraps silently over 999.
fn u64_to_dec3(value: u64, buffer: &mut [u8], pos: usize) {
    buffer[pos] = b'0' + ((value / 100) % 10) as u8;
    buffer[pos + 1] = b'0' + ((value / 10) % 10) as u8;
    buffer[pos + 2] = b'0' + (value % 10) as u8;
}

/// Formats one CAN frame as an Actisense log line, and returns its length.
///
/// `HH:MM:SS.mmm R CANID D0 D1 D2 D3 D4 D5 D6 D7\r\n`
///
/// The timestamp is uptime, not wall clock, and it wraps at 24 hours. A full
/// 8-byte frame writes 49 bytes, so the 128-byte buffer never overflows.
pub fn format_actisense(frame: &CanFrame, uptime_ms: u64, buffer: &mut [u8; 128]) -> usize {
    let total_seconds = uptime_ms / 1000;
    let milliseconds = uptime_ms % 1000;
    let hours = ((total_seconds / 3600) % 24) as u8;
    let minutes = ((total_seconds / 60) % 60) as u8;
    let seconds = (total_seconds % 60) as u8;

    let can_id = frame.id.0; // the inner u32

    let mut pos = 0;

    // HH:MM:SS.mmm
    u8_to_dec(hours, buffer, pos);
    pos += 2;
    buffer[pos] = b':';
    pos += 1;
    u8_to_dec(minutes, buffer, pos);
    pos += 2;
    buffer[pos] = b':';
    pos += 1;
    u8_to_dec(seconds, buffer, pos);
    pos += 2;
    buffer[pos] = b'.';
    pos += 1;
    u64_to_dec3(milliseconds, buffer, pos);
    pos += 3;

    // " R "
    buffer[pos] = b' ';
    pos += 1;
    buffer[pos] = b'R';
    pos += 1;
    buffer[pos] = b' ';
    pos += 1;

    // CAN ID, eight hex digits
    u32_to_hex(can_id, buffer, pos);
    pos += 8;

    // Data bytes
    for i in 0..frame.len {
        buffer[pos] = b' ';
        pos += 1;
        u8_to_hex(frame.data[i], buffer, pos);
        pos += 2;
    }

    // CRLF: some readers expect it, none refuse it.
    buffer[pos] = b'\r';
    pos += 1;
    buffer[pos] = b'\n';
    pos += 1;

    pos
}

//================================================================================= ACTISENSE NGT-1

/// Actisense NGT-1 protocol constants.
pub const DLE: u8 = 0x10;
pub const STX: u8 = 0x02;
pub const ETX: u8 = 0x03;
/// N2K data message. SignalK sends 0x94.
pub const MSG_N2K_DATA: u8 = 0x94;

/// Parses one binary Actisense NGT-1 message into a [`CanFrame`].
///
/// Wire format: `[DLE STX cmd len data... checksum DLE ETX]`.
/// Returns `None` on anything that is not a complete N2K data message.
///
/// The checksum is not verified: the USB link that carries these bytes already
/// checks them, and a bad frame fails later at decode.
pub fn parse_ngt1_message(data: &[u8]) -> Option<CanFrame> {
    // Shortest possible message: DLE STX cmd len ... DLE ETX
    if data.len() < 6 {
        return None;
    }

    // Find the DLE STX that opens the message.
    let start = data.windows(2).position(|w| w[0] == DLE && w[1] == STX)?;

    if start + 4 >= data.len() {
        return None;
    }

    let cmd = data[start + 2];

    // N2K data messages only. Everything else is control traffic.
    if cmd != MSG_N2K_DATA {
        return None;
    }

    let msg_len = data[start + 3] as usize;

    // The announced length must actually be there.
    if start + 4 + msg_len + 2 > data.len() {
        return None;
    }

    // Unescape the payload as it is copied out.
    let mut payload = [0u8; 32];
    let mut payload_pos = 0;
    let mut i = start + 4;
    let end = start + 4 + msg_len;

    while i < end && payload_pos < payload.len() {
        if data[i] == DLE && i + 1 < end && data[i + 1] == DLE {
            // Escaped DLE: DLE DLE means one DLE.
            payload[payload_pos] = DLE;
            payload_pos += 1;
            i += 2;
        } else {
            payload[payload_pos] = data[i];
            payload_pos += 1;
            i += 1;
        }
    }

    // The SignalK NGT-1 payload is [priority] [PGN:3] [dst] [len] [data...].
    // No source address: it lives in the NGT-1 header, or is implicit.
    if payload_pos < 6 {
        defmt::info!("NGT-1 parse: payload too short ({} bytes)", payload_pos);
        return None;
    }

    let priority = payload[0];
    let pgn = ((payload[3] as u32) << 16) | ((payload[2] as u32) << 8) | (payload[1] as u32);
    let dst = payload[4];
    let data_len = payload[5] as usize;

    defmt::info!(
        "NGT-1 parse: prio={}, PGN={}, dst={}, len={}",
        priority,
        pgn,
        dst,
        data_len
    );

    if payload_pos < 6 + data_len {
        defmt::warn!(
            "NGT-1 parse: not enough data ({} < {})",
            payload_pos,
            6 + data_len
        );
        return None;
    }

    // Broadcast: SignalK never sends a source address.
    let src = 255u8;

    // Build the CAN ID.
    let can_id = CanId::builder(pgn, src)
        .with_priority(priority)
        .build()
        .ok()?;

    // A CAN frame carries 8 bytes at most. Anything longer is Fast Packet and
    // is truncated here, not reassembled.
    let mut frame_data = [0u8; 8];
    let frame_len = data_len.min(8);
    frame_data[..frame_len].copy_from_slice(&payload[6..6 + frame_len]);

    Some(CanFrame {
        id: can_id,
        data: frame_data,
        len: frame_len,
    })
}
