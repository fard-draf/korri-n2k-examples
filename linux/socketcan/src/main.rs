use anyhow::Result;
use chrono::Local;
use korri_n2k::protocol::transport::can_frame::CanFrame;
use socketcan::{CanFrame as LinuxCanFrame, CanSocket, EmbeddedFrame, Frame, Socket};
use std::io::{self, Write};
use std::time::Instant;

/// Formatage Actisense adapté pour Linux
/// Format: HH:MM:SS.mmm R CANID D0 D1 D2 D3 D4 D5 D6 D7
fn format_actisense(frame: &LinuxCanFrame, start_instant: Instant) -> String {
    let now = Local::now();
    let uptime = start_instant.elapsed();

    // On utilise chrono pour l'heure système réelle,
    // ou on peut simuler l'uptime comme dans l'exemple original.
    let timestamp = now.format("%H:%M:%S%.3f").to_string();

    let can_id = frame.can_id().as_raw();

    let mut data_str = String::new();
    for byte in frame.data() {
        data_str.push_str(&format!(" {:02X}", byte));
    }

    format!("{} R {:08X}{}\r\n", timestamp, can_id, data_str)
}

fn main() -> Result<()> {
    // Sur BBB, l'interface est généralement can0 ou can1
    // Tu peux tester avec "vcan0" sur ton PC
    let interface = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "can0".to_string());

    println!("Démarrage du récepteur sur {}...", interface);

    let socket = CanSocket::open(&interface)
        .map_err(|e| anyhow::anyhow!("Impossible d'ouvrir {}: {}", interface, e))?;

    let start_time = Instant::now();
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    println!("Prêt à recevoir (Format Actisense)...");

    loop {
        match socket.read_frame() {
            Ok(frame) => {
                let output = format_actisense(&frame, start_time);
                let _ = handle.write_all(output.as_bytes());
                let _ = handle.flush();
            }
            Err(e) => {
                eprintln!("Erreur de lecture CAN: {:?}", e);
            }
        }
    }
}
