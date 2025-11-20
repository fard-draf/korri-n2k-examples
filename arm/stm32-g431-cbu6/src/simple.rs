#![no_std]
#![no_main]

use defmt::{info, warn, error};
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_stm32::{
    self as hal,
    gpio::{Level, Output, Speed},
    time::Hertz,
    rcc::{self, Hse, HseMode, Pll, PllMul, PllPreDiv, PllQDiv, PllRDiv, Sysclk},
    peripherals,
    can::{self, CanConfigurator, OperatingMode},
};
use embassy_time::{Timer, Duration};

const N2K_BITRATE: u32 = 250_000;

hal::bind_interrupts!(struct CanIrqs {
    FDCAN1_IT0 => can::IT0InterruptHandler<peripherals::FDCAN1>;
    FDCAN1_IT1 => can::IT1InterruptHandler<peripherals::FDCAN1>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // RCC minimal pour HSE 8 MHz + FDCAN sur HSE
    let mut cfg = hal::Config::default();
    cfg.rcc.hse = Some(Hse {
        freq: Hertz(8_000_000),
        mode: HseMode::Oscillator,
    });
    cfg.rcc.sys = Sysclk::PLL1_R;
    cfg.rcc.pll = Some(Pll {
        source: rcc::PllSource::HSE,
        prediv: PllPreDiv::DIV1,
        mul: PllMul::MUL40,
        divp: None,
        divq: Some(PllQDiv::DIV8),
        divr: Some(PllRDiv::DIV2),
    });
    cfg.rcc.mux.fdcansel = rcc::mux::Fdcansel::HSE;

    let p = hal::init(cfg);

    let mut led = Output::new(p.PA6, Level::Low, Speed::High);

    let mut can = init_fdcan(p.FDCAN1, p.PA11, p.PA12, CanIrqs);

    // Envoi périodique d’une trame pour voir si tout tourne
    let id = embedded_can::StandardId::new(0x123).unwrap();
    let frame = can::Frame::new_data(id, &[1, 2, 3, 4]).unwrap();
    
    loop {
        led.toggle();
        let dropped = can.write(&frame).await;
        Timer::after(Duration::from_millis(100)).await;
        match dropped {
            Some(_old) => {
                warn!("TX: frame envoyée, une ancienne trame a été drop");
            }
            None => {
                info!("TX: frame envoyée (aucun drop)");
            }
        }
    }
}

fn init_fdcan<'d>(
    fdcan: hal::Peri<'d, peripherals::FDCAN1>,
    rx: hal::Peri<'d, peripherals::PA11>,
    tx: hal::Peri<'d, peripherals::PA12>,
    irqs: CanIrqs,
) -> hal::can::Can<'d> {
    let mut cfg = CanConfigurator::new(fdcan, rx, tx, irqs);
    cfg.set_bitrate(N2K_BITRATE);
    cfg.start(OperatingMode::NormalOperationMode)
}
