#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use korri_n2k::protocol::transport::{can_frame::CanFrame, can_id::CanId, traits::can_bus::CanBus};

use defmt_rtt as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    timer::timg::TimerGroup,
    twai::{self, BaudRate, TwaiMode},
    uart::{self, Uart},
};
use esp_println::println;

esp_bootloader_esp_idf::esp_app_desc!();

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    esp_hal::system::software_reset()
}

/// Constantes du protocole Actisense NGT-1
const DLE: u8 = 0x10;
const STX: u8 = 0x02;
const ETX: u8 = 0x03;
const MSG_N2K_DATA: u8 = 0x94; // Message N2K data (0x94 pour SignalK)

/// Parse un message binaire Actisense NGT-1 en CanFrame
/// Format: [DLE STX cmd len data... checksum DLE ETX]
fn parse_ngt1_message(data: &[u8]) -> Option<CanFrame> {
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
        println!("NGT-1 parse: payload too short ({} bytes)", payload_pos);
        return None;
    }

    let priority = payload[0];
    let pgn = ((payload[3] as u32) << 16) | ((payload[2] as u32) << 8) | (payload[1] as u32);
    let dst = payload[4];
    let data_len = payload[5] as usize;

    println!(
        "NGT-1 parse: prio={}, PGN={}, dst={}, len={}",
        priority, pgn, dst, data_len
    );

    if payload_pos < 6 + data_len {
        println!(
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

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    println!("FEED - Init async..");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    // Configuration UART pour lire les données ACTISENSE
    let uart_config = uart::Config::default().with_baudrate(115200);
    let uart = Uart::new(peripherals.UART0, uart_config)
        .expect("Failed to create UART")
        .into_async();
    let (mut uart_rx, _) = uart.split();

    let can_tx_pin = peripherals.GPIO17; //vert
    let can_rx_pin = peripherals.GPIO18; //bleu

    const TWAI_BAUDRATE: twai::BaudRate = BaudRate::B250K;

    let can_config = twai::TwaiConfiguration::new(
        peripherals.TWAI0,
        can_rx_pin,
        can_tx_pin,
        TWAI_BAUDRATE,
        TwaiMode::Normal,
    )
    .into_async();

    let can_peripheral = can_config.start();
    let mut can = korri_bsp_esp32::ports::EspCanBus::new(can_peripheral);
    println!("TWAI async started, ready to feed from UART..");

    led.set_high();
    Timer::after(Duration::from_millis(1000)).await;
    led.set_low();

    let mut msg_buffer = [0u8; 256];
    let mut msg_pos = 0;
    let mut total_sent = 0;
    let mut parse_errors = 0;
    let mut byte_count = 0;
    let mut in_message = false;

    println!("Ready to receive NGT-1 binary data from UART..");
    loop {
        // Lire un byte depuis l'UART
        let mut byte = [0u8; 1];
        match uart_rx.read_async(&mut byte).await {
            Ok(_) => {
                let c = byte[0];
                byte_count += 1;

                // Debug: afficher tous les 1000 bytes
                if byte_count % 1000 == 0 {
                    println!(
                        "RX: {} bytes, {} sent, {} errors",
                        byte_count, total_sent, parse_errors
                    );
                }

                // Détecter le début d'un message: DLE STX
                if c == DLE && !in_message {
                    msg_buffer[0] = c;
                    msg_pos = 1;
                    in_message = true;
                } else if in_message {
                    if msg_pos < msg_buffer.len() {
                        msg_buffer[msg_pos] = c;
                        msg_pos += 1;

                        // Détecter la fin: DLE ETX
                        if msg_pos >= 2 && msg_buffer[msg_pos - 2] == DLE && c == ETX {
                            // Message complet, afficher pour debug
                            println!(
                                "NGT-1 msg ({} bytes): {:?}",
                                msg_pos,
                                &msg_buffer[..msg_pos.min(20)]
                            );

                            // Parser
                            if let Some(frame) = parse_ngt1_message(&msg_buffer[..msg_pos]) {
                                // Envoyer sur le bus CAN
                                match can.send(&frame).await {
                                    Ok(_) => {
                                        total_sent += 1;
                                        println!(
                                            ">>> Sent to CAN: PGN={}, SA={}, len={}",
                                            frame.id.pgn(),
                                            frame.id.source_address(),
                                            frame.len
                                        );

                                        // Blink LED tous les 10 messages
                                        if total_sent % 10 == 0 {
                                            led.set_high();
                                            Timer::after(Duration::from_millis(20)).await;
                                            led.set_low();
                                        }
                                    }
                                    Err(_) => {
                                        println!("ERROR: Failed to send to CAN");
                                    }
                                }
                            } else {
                                parse_errors += 1;
                                println!(
                                    "PARSE ERROR #{}: cmd={:02X}, len={}",
                                    parse_errors,
                                    if msg_pos > 2 { msg_buffer[2] } else { 0 },
                                    if msg_pos > 3 { msg_buffer[3] } else { 0 }
                                );
                            }

                            // Reset pour le prochain message
                            msg_pos = 0;
                            in_message = false;
                        }
                    } else {
                        // Buffer overflow
                        println!("BUFFER OVERFLOW - message too long, resetting");
                        msg_pos = 0;
                        in_message = false;
                    }
                }
            }
            Err(_) => {
                // Erreur UART, continuer
                Timer::after(Duration::from_millis(10)).await;
            }
        }
    }
}
