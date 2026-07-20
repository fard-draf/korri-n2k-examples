use defmt::info;
use defmt_rtt as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    peripherals::{GPIO41, GPIO42, TWAI0},
    timer::timg::TimerGroup,
    twai::{Twai, TwaiConfiguration, TwaiMode},
    Async,
};

use crate::conf::*;

/// Peripherals the BSP hands over to the N2K layer.
///
/// `esp_hal::init` returns a `Peripherals` struct whose fields move out one by
/// one, so it cannot be passed on whole once TIMG0 has been taken for the
/// Embassy time driver. Only what the bus needs is extracted.
pub struct BspPeripherals {
    pub twai: TWAI0<'static>,
    pub rx: GPIO41<'static>,
    pub tx: GPIO42<'static>,
}

/// Max CPU clock, then start the Embassy time driver on TIMG0.
pub fn init_bsp() -> BspPeripherals {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    BspPeripherals {
        twai: peripherals.TWAI0,
        rx: peripherals.GPIO41,
        tx: peripherals.GPIO42,
    }
}

/// Starts the TWAI controller in normal mode on the N2K bus.
///
/// Unlike the STM32 FDCAN, esp-hal has no buffered driver: the 3-frame hardware
/// TX buffer is driven directly by `EspCanBus`.
pub fn init_can(p: BspPeripherals) -> Twai<'static, Async> {
    let config =
        TwaiConfiguration::new(p.twai, p.rx, p.tx, N2K_BITRATE, TwaiMode::Normal).into_async();

    let can = config.start();
    info!("TWAI initialised (250 kbps) on GPIO41(rx)/GPIO42(tx)");
    can
}
