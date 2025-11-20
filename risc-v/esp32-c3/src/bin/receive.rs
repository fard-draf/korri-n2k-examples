#![no_std]
#![no_main]

use embassy_executor::Spawner;
use korri_n2k::protocol::transport::{can_frame::CanFrame, can_id::CanId, traits::can_bus::CanBus};

use esp_backtrace as _;
use esp_println as _;
use esp_hal::{
    clock::CpuClock,
    time::Instant,
    gpio::{Level, Output, OutputConfig},
    timer::timg::TimerGroup,
    twai::{self, BaudRate, TwaiMode},
    uart::{self, Uart},
};

esp_bootloader_esp_idf::esp_app_desc!();
// You need a panic handler. Usually, you you would use esp_backtrace, panic-probe, or
// something similar, but you can also bring your own like this:
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    esp_hal::system::software_reset()
}

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
fn format_actisense(frame: &CanFrame, uptime_ms: u64, buffer: &mut [u8; 128]) -> usize {
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

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    // println!("RECEIVER - Init async..");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut led = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    // Configuration UART1 pour sortie ACTISENSE (UART0 est utilisé par esp-println)
    // GPIO4 = UART1_TX, GPIO5 = UART1_RX (par défaut ESP32-C3)
    let uart_config = uart::Config::default().with_baudrate(115200);
    let uart = Uart::new(peripherals.UART0, uart_config)
        .expect("Failed to create UART")
        .into_async();
    let (_, mut uart_tx) = uart.split();

    let can_tx_pin = peripherals.GPIO9; //bleu
    let can_rx_pin = peripherals.GPIO10; //vert

    const TWAI_BAUDRATE: twai::BaudRate = BaudRate::B250K;

    // Configuration avec filtre d'acceptation pour TOUTES les trames Extended ID (NMEA2000)
    let can_config = twai::TwaiConfiguration::new(
        peripherals.TWAI0,
        can_rx_pin,
        can_tx_pin,
        TWAI_BAUDRATE,
        TwaiMode::Normal,
    );

    // Configurer le filtre pour accepter toutes les trames
    // NMEA2000 utilise des Extended IDs (29 bits)
    let can_config = can_config.into_async();

    let can_peripheral = can_config.start();
    let mut can = korri_bsp_esp32_c3::ports::EspCanBus::new(can_peripheral);
    // println!("TWAI async started with CanBus trait..");

    // WORKAROUND ESP32-C3: Envoyer une trame au démarrage pour activer le contrôleur TWAI
    // Certains contrôleurs CAN nécessitent une transmission avant de recevoir correctement
    let dummy_frame = CanFrame {
        id: CanId(0x1FFFFFFF), // ID invalide qui ne sera pas traité
        data: [0xFF; 8],
        len: 0, // Longueur 0 = trame vide
    };
    let _ = can.send(&dummy_frame).await; // On ignore l'erreur


    let mut count = 0;
    let mut total_count = 0;
    let mut error_count = 0;
    let mut total_rx_time = 0u64;
    let mut actisense_buffer = [0u8; 128];
    let start_time = Instant::now();

    // println!("Ready to listen..");
    loop {
        let rx_start = Instant::now();
        match can.recv().await {
            Ok(frame) => {
                led.set_high();
                defmt::println!("PGN={} SA={}", frame.id.pgn(), frame.id.source_address());
                let rx_elapsed = (Instant::now() - rx_start).as_micros();
                total_rx_time += rx_elapsed;
                count += 1;
                total_count += 1;
                // Calculer l'uptime en millisecondes
                let uptime_ms = (Instant::now() - start_time).as_millis();

                // Formater et envoyer vers UART au format ACTISENSE
                let len = format_actisense(&frame, uptime_ms, &mut actisense_buffer);

                // Écrire tous les octets (boucle jusqu'à ce que tout soit écrit)
                let mut written = 0;
                while written < len {
                    match uart_tx.write_async(&actisense_buffer[written..len]).await {
                        Ok(n) => written += n,
                        Err(_) => break,
                    }
                }
                led.set_low();
            }
            Err(_) => {
            defmt::println!("ERROR !!!");
                error_count += 1;
            }
        }

        if error_count > 10 {
            defmt::println!("ERROR !!!");
        }
    }
}
