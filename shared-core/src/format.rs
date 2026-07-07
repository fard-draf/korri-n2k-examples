use korri_n2k::protocol::transport::{can_frame::CanFrame, can_id::CanId};

/// Convertit un nombre en 2 chiffres hexa dans le buffer
fn u8_to_hex(value: u8, buffer: &mut [u8], pos: usize) {
    const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";
    buffer[pos] = HEX_CHARS[(value >> 4) as usize];
    buffer[pos + 1] = HEX_CHARS[(value & 0x0F) as usize];
}

/// Convertit un nombre en 8 chiffres hexa dans le buffer
fn u32_to_hex(value: u32, buffer: &mut [u8], pos: usize) {
    const HEX_CHARS: &[u8; 16] = b"0123456789ABCDEF";
    for i in 0..8 {
        let shift = (7 - i) * 4;
        let nibble = ((value >> shift) & 0x0F) as usize;
        buffer[pos + i] = HEX_CHARS[nibble];
    }
}

/// Convertit un nombre en 2 chiffres décimaux dans le buffer
fn u8_to_dec(value: u8, buffer: &mut [u8], pos: usize) {
    buffer[pos] = b'0' + (value / 10);
    buffer[pos + 1] = b'0' + (value % 10);
}

/// Convertit un nombre en 3 chiffres décimaux dans le buffer
fn u64_to_dec3(value: u64, buffer: &mut [u8], pos: usize) {
    buffer[pos] = b'0' + ((value / 100) % 10) as u8;
    buffer[pos + 1] = b'0' + ((value / 10) % 10) as u8;
    buffer[pos + 2] = b'0' + (value % 10) as u8;
}
/// Formate une frame CAN au format ACTISENSE
/// Format: HH:MM:SS.mmm R CANID D0 D1 D2 D3 D4 D5 D6 D7
pub fn format_actisense(frame: &CanFrame, uptime_ms: u64, buffer: &mut [u8; 128]) -> usize {
    let total_seconds = uptime_ms / 1000;
    let milliseconds = uptime_ms % 1000;
    let hours = ((total_seconds / 3600) % 24) as u8;
    let minutes = ((total_seconds / 60) % 60) as u8;
    let seconds = (total_seconds % 60) as u8;

    let can_id = frame.id.0; // Accès au u32 interne

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

    // CAN ID en hexa (8 chiffres)
    u32_to_hex(can_id, buffer, pos);
    pos += 8;

    // Octets de données
    for i in 0..frame.len {
        buffer[pos] = b' ';
        pos += 1;
        u8_to_hex(frame.data[i], buffer, pos);
        pos += 2;
    }

    // Retour à la ligne (CRLF pour compatibilité)
    buffer[pos] = b'\r';
    pos += 1;
    buffer[pos] = b'\n';
    pos += 1;

    pos
}

//================================================================================= ACTISENSE NGT-1

/// Constantes du protocole Actisense NGT-1
pub const DLE: u8 = 0x10;
pub const STX: u8 = 0x02;
pub const ETX: u8 = 0x03;
pub const MSG_N2K_DATA: u8 = 0x94; // Message N2K data (0x94 pour SignalK)

/// Parse un message binaire Actisense NGT-1 en CanFrame
/// Format: [DLE STX cmd len data... checksum DLE ETX]
pub fn parse_ngt1_message(data: &[u8]) -> Option<CanFrame> {
    // Vérifier la structure minimum: DLE STX cmd len ... DLE ETX
    if data.len() < 6 {
        return None;
    }

    // Chercher DLE STX au début
    let start = data.windows(2).position(|w| w[0] == DLE && w[1] == STX)?;

    if start + 4 >= data.len() {
        return None;
    }

    let cmd = data[start + 2];

    // On ne traite que les messages N2K data (0x93)
    if cmd != MSG_N2K_DATA {
        return None;
    }

    let msg_len = data[start + 3] as usize;

    // Vérifier qu'on a assez de données
    if start + 4 + msg_len + 2 > data.len() {
        return None;
    }

    // Extraire le payload (en gérant l'échappement DLE)
    let mut payload = [0u8; 32];
    let mut payload_pos = 0;
    let mut i = start + 4;
    let end = start + 4 + msg_len;

    while i < end && payload_pos < payload.len() {
        if data[i] == DLE && i + 1 < end && data[i + 1] == DLE {
            // DLE échappé (DLE DLE -> DLE)
            payload[payload_pos] = DLE;
            payload_pos += 1;
            i += 2;
        } else {
            payload[payload_pos] = data[i];
            payload_pos += 1;
            i += 1;
        }
    }

    // Le payload NGT-1 SignalK contient: [priority] [PGN:3] [dst] [len] [data...]
    // (PAS de source address - il est dans l'en-tête NGT-1 ou implicite)
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

    // Source address = 255 (broadcast) par défaut car SignalK ne l'envoie pas
    let src = 255u8;

    // Construire le CAN ID
    let can_id = CanId::builder(pgn, src)
        .with_priority(priority)
        .build()
        .ok()?;

    // Extraire les données (max 8 bytes pour CAN)
    let mut frame_data = [0u8; 8];
    let frame_len = data_len.min(8);
    frame_data[..frame_len].copy_from_slice(&payload[6..6 + frame_len]);

    Some(CanFrame {
        id: can_id,
        data: frame_data,
        len: frame_len,
    })
}
