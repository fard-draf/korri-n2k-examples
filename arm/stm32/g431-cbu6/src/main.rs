#![no_std]
#![no_main]

use defmt::{error, info, trace};
use defmt_rtt as _;
use embassy_sync::{blocking_mutex::{CriticalSectionMutex, raw::CriticalSectionRawMutex}, mutex::Mutex};
use korri_n2k::protocol::managment::address_manager::AddressManager;
use panic_probe as _;

mod ports;
use ports::{Stm32CanBus, Stm32Timer, snapshot_can_diagnostics};
use shared_core::pgns::position_129025;

type AddressManagerType = AddressManager<Stm32CanBus<'static, CAN_TX_BUF_DEPTH, CAN_RX_BUF_DEPTH>, Stm32Timer>;

const N2K_BITRATE: u32 = 250_000;
const CAN_TX_BUF_DEPTH: usize = 8;
const CAN_RX_BUF_DEPTH: usize = 16;
const FDCAN_SILENT_MODE: bool = false;
const PREFERRED_ADDRESS: u8 = 148;
const ISO_UNIQUE_NUMBER: u32 = 0x1ABCDE;
const ISO_MANUFACTURER_CODE: u16 = 229;
const ISO_DEVICE_FUNCTION: u8 = 145;
const ISO_DEVICE_CLASS: u8 = 75;
const ISO_DEVICE_INSTANCE: u8 = 1;
const ISO_SYSTEM_INSTANCE: u8 = 0;
const ISO_INDUSTRY_GROUP: u8 = 4;
const DIAG_PERIOD_MS: u64 = 1_000;

static MANAGER: static_cell::StaticCell<embassy_sync::mutex::Mutex<CriticalSectionRawMutex, AddressManagerType>> = static_cell::StaticCell::new();
static TX_BUF: static_cell::StaticCell<embassy_stm32::can::TxBuf<CAN_TX_BUF_DEPTH>> = static_cell::StaticCell::new();
static RX_BUF: static_cell::StaticCell<embassy_stm32::can::RxBuf<CAN_RX_BUF_DEPTH>> = static_cell::StaticCell::new();

embassy_stm32::bind_interrupts!(struct CanIrqs {
    FDCAN1_IT0 => embassy_stm32::can::IT0InterruptHandler<embassy_stm32::peripherals::FDCAN1>;
    FDCAN1_IT1 => embassy_stm32::can::IT1InterruptHandler<embassy_stm32::peripherals::FDCAN1>;
});

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {

    //==============================================================RCC
    let mut rcc_cfg = embassy_stm32::Config::default();

    rcc_cfg.rcc.hse = Some(embassy_stm32::rcc::Hse {
        freq: embassy_stm32::time::Hertz(8_000_000),
        mode: embassy_stm32::rcc::HseMode::Oscillator,
    });
    rcc_cfg.rcc.sys = embassy_stm32::rcc::Sysclk::PLL1_R;
    rcc_cfg.rcc.pll = Some(embassy_stm32::rcc::Pll {
        source: embassy_stm32::rcc::PllSource::HSE,
        prediv: embassy_stm32::rcc::PllPreDiv::DIV1,
        mul: embassy_stm32::rcc::PllMul::MUL40,
        divp: None,
        divq: Some(embassy_stm32::rcc::PllQDiv::DIV8),
        divr: Some(embassy_stm32::rcc::PllRDiv::DIV2),
    });

    rcc_cfg.rcc.ahb_pre = embassy_stm32::rcc::AHBPrescaler::DIV1;
    rcc_cfg.rcc.apb1_pre = embassy_stm32::rcc::APBPrescaler::DIV2;
    rcc_cfg.rcc.apb2_pre = embassy_stm32::rcc::APBPrescaler::DIV1;
    rcc_cfg.rcc.mux.fdcansel = embassy_stm32::rcc::mux::Fdcansel::HSE;
    rcc_cfg.rcc.boost = true;

    //==============================================================INIT HAL + PERI
    let p = embassy_stm32::init(rcc_cfg);

    //==============================================================INIT CAN
    let irqs = CanIrqs;
    let can = init_fdcan(p.FDCAN1, p.PA11, p.PA12, irqs);
    info!(
        "FDCAN initialisé ({} kbps). Err TX {}, Err RX {}",
        N2K_BITRATE / 1000,
        can.properties().tx_error_count(),
        can.properties().rx_error_count()
    );

    info!("Embassy up. Bringing up korri-n2k stack.");

    let iso_name = korri_n2k::protocol::managment::iso_name::IsoName::builder()
        .unique_number(ISO_UNIQUE_NUMBER)
        .manufacturer_code(ISO_MANUFACTURER_CODE)
        .device_function(ISO_DEVICE_FUNCTION)
        .device_class(ISO_DEVICE_CLASS)
        .device_instance(ISO_DEVICE_INSTANCE)
        .system_instance(ISO_SYSTEM_INSTANCE)
        .industry_group(ISO_INDUSTRY_GROUP)
        .arbitrary_address_capable(true)
        .build();
    info!("ISO NAME: 0x{=u64:X}", iso_name.raw());

    //==============================================================CLOCKS DIAG
    let clocks = embassy_stm32::rcc::clocks(&p.RCC);
    let sys_hz = clocks.sys.to_hertz().map(|hz| hz.0).unwrap_or(0);
    let pll1_q_hz = clocks.pll1_q.to_hertz().map(|hz| hz.0).unwrap_or(0);
    info!("RCC clocks sys={}Hz pll1_q={}Hz", sys_hz, pll1_q_hz);

    //==============================================================INIT MANAGER
    let manager = match korri_n2k::protocol::managment::address_manager::AddressManager::new(
        Stm32CanBus::new(can),
        Stm32Timer::new(),
        iso_name.raw(),
        PREFERRED_ADDRESS,
    )
    .await
    {
        Ok(mgr) => {
            info!("Address claimed: {}", mgr.current_address());
            mgr
        }
        Err(_) => {
            error!("Failed to claim an address on the NMEA2000 bus");
            panic!("address claim failed");
        }
    };

    let manager_ref = MANAGER.init(embassy_sync::mutex::Mutex::new(manager));

    //==============================================================SPAWNER
    spawner
        .spawn(can_diagnostics_task())
        .expect("spawn CAN diag");

    spawner
        .spawn(ac_input_task(manager_ref))
        .expect("spawn ac_input task");

    spawner
        .spawn(position_task(manager_ref))
        .expect("spawn position task");

    spawner
        .spawn(engine_488_task(manager_ref))
        .expect("spawn engine_488 task");
    info!("Korri stack ready; tasks running.");

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(5)).await;
        trace!("main idle");
    }
}


#[embassy_executor::task]
async fn can_diagnostics_task() {
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_millis(DIAG_PERIOD_MS)).await;
        let diag = snapshot_can_diagnostics();
        defmt::info!("FDCAN {:?}", diag);
    }
}

fn init_fdcan<'d>(
    fdcan: embassy_stm32::Peri<'d, embassy_stm32::peripherals::FDCAN1>,
    rx: embassy_stm32::Peri<'d, embassy_stm32::peripherals::PA11>,
    tx: embassy_stm32::Peri<'d, embassy_stm32::peripherals::PA12>,
    irqs: CanIrqs,
) -> embassy_stm32::can::BufferedCan<'d, CAN_TX_BUF_DEPTH, CAN_RX_BUF_DEPTH> {
    let mut cfg = embassy_stm32::can::CanConfigurator::new(fdcan, rx, tx, irqs);
    cfg.set_bitrate(N2K_BITRATE);
    cfg.properties()
        .set_standard_filter(embassy_stm32::can::filter::StandardFilterSlot::_0, embassy_stm32::can::filter::StandardFilter::reject_all());
    cfg.properties().set_extended_filter(
        embassy_stm32::can::filter::ExtendedFilterSlot::_0,
        embassy_stm32::can::filter::ExtendedFilter::accept_all_into_fifo0(),
    );


    let mode = if FDCAN_SILENT_MODE {
        embassy_stm32::can::OperatingMode::InternalLoopbackMode
    } else {
        embassy_stm32::can::OperatingMode::NormalOperationMode
    };
    info!("FDCAN silent mode: {}", FDCAN_SILENT_MODE);
    let can = cfg.start(mode);

    let tx_buf = TX_BUF.init(embassy_stm32::can::TxBuf::new());
    let rx_buf = RX_BUF.init(embassy_stm32::can::RxBuf::new());

    can.buffered(tx_buf, rx_buf)
}

#[embassy_executor::task]
async fn ac_input_task(manager: &'static embassy_sync::mutex::Mutex<CriticalSectionRawMutex, AddressManagerType>) {
    shared_core::pgns::ac_input_127503::task_ac_input_127503(manager).await;
}

#[embassy_executor::task]
async fn position_task(manager: &'static embassy_sync::mutex::Mutex<CriticalSectionRawMutex, AddressManagerType>) {
    shared_core::pgns::position_129025::task_position_129025(manager).await;
}

#[embassy_executor::task]
async fn engine_488_task(manager: &'static embassy_sync::mutex::Mutex<CriticalSectionRawMutex, AddressManagerType>) {
    shared_core::pgns::engine_127488::task_engine_127488(manager).await;
}
