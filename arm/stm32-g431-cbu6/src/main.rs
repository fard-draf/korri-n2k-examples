#![no_std]
#![no_main]

use defmt::{error, info, trace};
use defmt_rtt as _;
use panic_probe as _;

mod manager_service;
mod ports;
use manager_service::ManagerHandle;
use ports::{Stm32CanBus, Stm32Timer, snapshot_can_diagnostics};

// Débit nominal du bus NMEA 2000 (250 kb/s). Utilisé pour calculer la
// temporisation FDCAN via `CanConfigurator::set_bitrate`.
const N2K_BITRATE: u32 = 250_000;
// Profondeur du buffer logiciel TX (nombre de trames en attente avant l’envoi hardware).
const CAN_TX_BUF_DEPTH: usize = 8;
// Taille du FIFO logiciel RX pour lisser les rafales entrantes.
const CAN_RX_BUF_DEPTH: usize = 16;
// OPERATION MODE
const FDCAN_SILENT_MODE: bool = false;
// N2K CONF
const PREFERRED_ADDRESS: u8 = 148;
const ISO_UNIQUE_NUMBER: u32 = 0x1ABCDE;
const ISO_MANUFACTURER_CODE: u16 = 229;
const ISO_DEVICE_FUNCTION: u8 = 145;
const ISO_DEVICE_CLASS: u8 = 75;
const ISO_DEVICE_INSTANCE: u8 = 1;
const ISO_SYSTEM_INSTANCE: u8 = 0;
const ISO_INDUSTRY_GROUP: u8 = 4;
const DIAG_PERIOD_MS: u64 = 1_000;

// Stockage statique (initialisé au runtime) pour la file TX, afin d’éviter `static mut`.
static TX_BUF: static_cell::StaticCell<embassy_stm32::can::TxBuf<CAN_TX_BUF_DEPTH>> = static_cell::StaticCell::new();
// Stockage statique (initialisé au runtime) pour la file RX.
static RX_BUF: static_cell::StaticCell<embassy_stm32::can::RxBuf<CAN_RX_BUF_DEPTH>> = static_cell::StaticCell::new();

// Associe les interruptions FDCAN1 aux handlers Embassy pour réveiller le driver async.
// ================================================================================== INTERRUPTIONS
embassy_stm32::bind_interrupts!(struct CanIrqs {
    FDCAN1_IT0 => embassy_stm32::can::IT0InterruptHandler<embassy_stm32::peripherals::FDCAN1>;
    FDCAN1_IT1 => embassy_stm32::can::IT1InterruptHandler<embassy_stm32::peripherals::FDCAN1>;
});

// Point d’entrée async: configure les horloges, lance les tâches et vérifie que
// le contrôleur CAN est prêt avant d’animer la LED principale.
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

    //==============================================================OUTPUT LED
    let led = embassy_stm32::gpio::Output::new(p.PA6, embassy_stm32::gpio::Level::Low, embassy_stm32::gpio::Speed::High);
    // unsafe {
    //     cortex_m::peripheral::NVIC::unmask(embassy_stm32::interrupt::FDCAN1_IT0);
    //     cortex_m::peripheral::NVIC::unmask(embassy_stm32::interrupt::FDCAN1_IT1);
    // }

    // embassy_stm32::interrupt::InterruptExt::set_priority(
    //     embassy_stm32::interrupt::FDCAN1_IT0,
    //     embassy_stm32::interrupt::Priority::P4,
    // );
    // embassy_stm32::interrupt::InterruptExt::set_priority(
    //     embassy_stm32::interrupt::FDCAN1_IT1,
    //     embassy_stm32::interrupt::Priority::P4,
    // );

    //==============================================================INIT CAN
    // `CanIrqs` est un type sans état généré par `bind_interrupts!` qui indique
    // au driver quelles routines serviront FDCAN1_IT0/IT1. On le passe ensuite
    // à `CanConfigurator` pour que l’IRQ soit correctement branchée.
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

    let (runner, mgr_handle) = manager_service::init_manager(manager);

    //==============================================================SPAWNER
    spawner
        .spawn(manager_service::address_manager_task(runner))
        .expect("spawn address manager");

    spawner
        .spawn(can_diagnostics_task())
        .expect("spawn CAN diag");
    spawner
        .spawn(task_position_129025(mgr_handle, led))
        .expect("spawn PGN129025");

    info!("Korri stack ready; tasks running.");

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(5)).await;
        trace!("main idle");
    }
}

// Tâche secondaire qui pulse une LED pour signaler que l’exécuteur tourne bien.
#[embassy_executor::task]
async fn heartbeat(mut led: embassy_stm32::gpio::Output<'static>) {
    loop {
        trace!("spawn: heartbeat");
        led.toggle();
        embassy_time::Timer::after(embassy_time::Duration::from_millis(500)).await;
    }
}
#[embassy_executor::task]
pub async fn task_position_129025(
    handle: &'static ManagerHandle,
    mut led: embassy_stm32::gpio::Output<'static>,
) {
    let mut ticker = embassy_time::Ticker::every(embassy_time::Duration::from_secs(1));
    loop {
        led.set_high();
        ticker.next().await;
        // Timer::after_millis(1000).await;
        let mut position_pgn = korri_n2k::protocol::messages::Pgn129025::new();
        position_pgn.latitude = 01.000;
        position_pgn.longitude = 20.000;
        trace!("spawn: task_position");
        if let Err(_err) = handle.send_pgn(&position_pgn, 129025, 2, None).await {
            defmt::warn!("Failed to enqueue PGN 129025");
            defmt::info!("error task_position");
        } else {
            defmt::info!("PGN 129025 queued");
        }
        led.set_low();
    }
}

// #[embassy_executor::task]
// async fn can_receive(
//     handler: &'static ManagerHandle,
//     mut indicator: embassy_stm32::gpio::Output<'static>,
// ) {
//     info!("Listening for NMEA2000 traffic …");

//     loop {
//         match handler.send_frame().await {
//             Ok(Some(frame)) => {
//                 indicator.toggle();
//                 info!(
//                     "RX PGN {=u32} from {=u8} len {=usize}",
//                     frame.id.pgn(),
//                     frame.id.source_address(),
//                     frame.len
//                 );
//                 trace!("payload: {=[u8]}", &frame.data[..frame.len]);
//             }
//             Ok(None) => {
//                 info!("RX PGN Received -> Nothing to print");
//             }
//             Err(err) => {
//                 error!("CAN receive error: {}", err);
//                 Timer::after(Duration::from_millis(100)).await;
//             }
//         }
//     }
// }
#[embassy_executor::task]
async fn can_diagnostics_task() {
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_millis(DIAG_PERIOD_MS)).await;
        let diag = snapshot_can_diagnostics();
        defmt::info!("FDCAN {:?}", diag);
    }
}

// Configure et démarre FDCAN1 en mode NMEA 2000 (250 kb/s, filtrage étendu).
//
// - affecte les broches PA11/PA12 au contrôleur
// - applique un filtre global strict (rejette les trames standard/remote)
// - installe un filtre 29 bits “accept all” sur FIFO0 pour laisser `korri-n2k`
//   gérer le dispatch logiciel
// - retourne une instance `BufferedCan` prête à être partagée entre tâches.
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

    // let mut can_config = cfg.config();
    // can_config = can_config.set_frame_transmit(embassy_stm32::can::config::FrameTransmissionConfig::ClassicCanOnly);
    // can_config = can_config.set_global_filter(
    //     embassy_stm32::can::config::GlobalFilter::default()
    //         .set_handle_standard_frames(embassy_stm32::can::config::NonMatchingFilter::Reject)
    //                              .set_handle_extended_frames(embassy_stm32::can::config::NonMatchingFilter::IntoRxFifo0)
    //                              .set_reject_remote_standard_frames(false)
    //                              .set_reject_remote_extended_frames(true),
    // );

    // can_config = can_config.set_automatic_retransmit(false);
    // cfg.set_config(can_config);

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

