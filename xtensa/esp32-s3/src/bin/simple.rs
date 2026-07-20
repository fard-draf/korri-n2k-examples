#![no_std]
#![no_main]

use defmt::{info, warn};
use defmt_rtt as _;
use esp_backtrace as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::CpuClock,
    timer::timg::TimerGroup,
    twai::{BaudRate, EspTwaiFrame, StandardId, TwaiConfiguration, TwaiMode},
};

esp_bootloader_esp_idf::esp_app_desc!();

const N2K_BITRATE: BaudRate = BaudRate::B250K;

/// Standalone TWAI smoke test, outside the korri-n2k stack.
#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let mut can = TwaiConfiguration::new(
        peripherals.TWAI0,
        peripherals.GPIO41,
        peripherals.GPIO42,
        N2K_BITRATE,
        TwaiMode::Normal,
    )
    .into_async()
    .start();

    // Send a frame periodically to check the bus is alive
    let id = StandardId::new(0x123).unwrap();
    let frame = EspTwaiFrame::new(id, &[1, 2, 3, 4]).unwrap();

    loop {
        match can.transmit_async(&frame).await {
            Ok(()) => info!("TX: frame sent"),
            Err(_) => warn!("TX: send failed (buffer full or silent bus)"),
        }
        Timer::after(Duration::from_millis(1500)).await;
    }
}
