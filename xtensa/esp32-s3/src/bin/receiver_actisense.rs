#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use korri_n2k::protocol::transport::traits::can_bus::CanBus;

use defmt::Debug2Format;
// use defmt_rtt as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    time::Instant,
    timer::timg::TimerGroup,
    twai::{self, BaudRate, TwaiMode},
    uart::{self, Uart},
};

use shared_core::format::format_actisense;
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    defmt::println!("RECEIVER - Init async..");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    // Configuration UART pour sortie ACTISENSE
    let uart_config = uart::Config::default().with_baudrate(115200);
    let uart = Uart::new(peripherals.UART0, uart_config)
        .expect("Failed to create UART")
        .into_async();
    let (_, mut uart_tx) = uart.split();

    let can_tx_pin = peripherals.GPIO42; //violet
    let can_rx_pin = peripherals.GPIO41; //orange

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
    let mut can = esp32_s3::ports::EspCanBus::new(can_peripheral);
    defmt::println!("TWAI async started with CanBus trait..");

    led.set_high();
    Timer::after(Duration::from_millis(1000)).await;
    led.set_low();

    // let mut count = 0;
    // let mut total_count = 0;
    // let mut error_count = 0;
    // let mut total_rx_time = 0u64;
    let mut actisense_buffer = [0u8; 128];
    let start_time = Instant::now();

    defmt::println!("Ready to listen..");
    loop {
        // let rx_start = Instant::now();

        match can.recv().await {
            Ok(frame) => {
                // let rx_elapsed = (Instant::now() - rx_start).as_micros();
                // total_rx_time += rx_elapsed;
                // count += 1;
                // total_count += 1;
                // Calculer l'uptime en millisecondes
                let uptime_ms = (Instant::now() - start_time).as_millis();
                defmt::trace!("{:?}", Debug2Format(&frame));
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
            }
            Err(_) => {
                // total_count += 1;
                // error_count += 1;
            }
        }
    }
}
