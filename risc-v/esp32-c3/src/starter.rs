use defmt::info;
use defmt_rtt as _;
use esp_backtrace as _;
use esp_hal::{
    Async,
    clock::CpuClock,
    peripherals::{GPIO9, GPIO10, TWAI0},
    timer::timg::TimerGroup,
    twai::{Twai, TwaiConfiguration, TwaiMode},
};

use crate::conf::*;

/// Peripherals the BSP hands over to the N2K layer.
///
/// `esp_hal::init` returns a `Peripherals` struct whose fields move out one by
/// one, so it cannot be passed on whole once TIMG0 has been taken for the
/// Embassy time driver. Only what the bus needs is extracted.
pub struct BspPeripherals {
    pub twai: TWAI0<'static>,
    pub rx: GPIO10<'static>,
    pub tx: GPIO9<'static>,
}

/// Max CPU clock, then start the Embassy time driver on TIMG0.
pub fn init_bsp() -> BspPeripherals {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    BspPeripherals {
        twai: peripherals.TWAI0,
        rx: peripherals.GPIO10,
        tx: peripherals.GPIO9,
    }
}

/// Starts the TWAI controller in normal mode on the N2K bus.
///
/// Like the ESP32-S3, esp-hal has no buffered driver here: the 3-frame hardware
/// TX buffer is driven directly by `EspCanBus`.
pub fn init_can(p: BspPeripherals) -> Twai<'static, Async> {
    let config =
        TwaiConfiguration::new(p.twai, p.rx, p.tx, N2K_BITRATE, TwaiMode::Normal).into_async();

    let can = config.start();
    info!("TWAI initialised (250 kbps) on GPIO10(rx)/GPIO9(tx)");
    can
}
