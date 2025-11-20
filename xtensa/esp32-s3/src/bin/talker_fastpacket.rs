#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    timer::timg::TimerGroup,
    twai::{BaudRate, TwaiMode},
};
use esp_println::println;
use korri_bsp_esp32::{
    pgns::{
        ac_input_127503::task_ac_input_127503, ais_class_a_129038::task_ais_class_a_129038,
        ais_class_b_129039::task_ais_class_b_129039, alert_text_126985::task_alert_text_126985,
        datum_129044::task_datum_129044, depth_128267::task_depth_128267,
        engine_127488::task_engine_127488, engine_127489::task_engine_127489,
        environmental_130310::task_environmental_130310,
        heading_control_127237::task_heading_control_127237,
        heartbeat_126993::task_heartbeat_126993, navigation_129284::task_navigation_129284,
        position_129025::task_position_129025, rudder_127245::task_rudder_127245,
        speed_128259::task_speed_128259,
    },
    ports::EspCanBus,
    timer::EspTimer,
};
use korri_n2k::{
    infra::codec::traits::{PgnData, ToPayload},
    protocol::{
        lookups::{ControllerState, EquipmentStatus, PgnErrorCode, WaterReference},
        managment::address_manager::AddressManager,
        messages::{Pgn126993, Pgn128259, Pgn128267, Pgn129025},
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
            traits::{can_bus, korri_timer::KorriTimer, pgn_sender::PgnSender},
        },
    },
};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();


#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use esp_println::println;
    println!("PANIC OCCURRED: {:?}", info);

    loop {
        for _ in 0..10_000_000 {
            core::sync::atomic::spin_loop_hint();
        }
    }
}

type AddressManagerType =
    AddressManager<korri_bsp_esp32::ports::EspCanBus<'static>, korri_bsp_esp32::timer::EspTimer>;

static MANAGER_CELL: StaticCell<Mutex<CriticalSectionRawMutex, AddressManagerType>> =
    StaticCell::new();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    defmt::info!("=== TALKER START ===");
    println!("TRANSMITER - Init async..");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let mut led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

    let can_tx_pin = peripherals.GPIO17; //vert
    let can_rx_pin = peripherals.GPIO18; //bleu

    let can_config = esp_hal::twai::TwaiConfiguration::new(
        peripherals.TWAI0,
        can_rx_pin,
        can_tx_pin,
        BaudRate::B250K,
        TwaiMode::Normal,
    )
    .into_async();

    let can_peripheral = can_config.start();
    let can_bus = korri_bsp_esp32::ports::EspCanBus::new(can_peripheral);

    let mut korri_timer = korri_bsp_esp32::timer::EspTimer;

    led.set_high();
    embassy_time::Timer::after(Duration::from_millis(1000)).await;
    led.set_low();

    let my_name = 0xFFF4567890ABCFFF;
    let preferred_address = 142;

    defmt::info!("Creating AddressManager...");

    // Créer l'AddressManager (fait le claim iniial automatiquement)
    let manager = match AddressManager::new(can_bus, korri_timer, my_name, preferred_address).await
    {
        Ok(mgr) => mgr,
        Err(_) => panic!("Failed to create AddressManager"),
    };

    defmt::info!("Address claimed: {}", manager.current_address());

    // Placer le manager dans un Mutex statique pour le partager entre les tâches
    let manager_mutex = MANAGER_CELL.init(Mutex::new(manager));

    spawner.spawn(test_ticker()).unwrap();
    // spawner.spawn(task_heartbeat_126993(manager_mutex)).unwrap();
    spawner.spawn(task_alert_text_126985(manager_mutex)).unwrap();
    // spawner.spawn(task_heading_control_127237(manager_mutex)).unwrap();
    spawner.spawn(task_ac_input_127503(manager_mutex)).unwrap();
    // spawner.spawn(task_ais_class_a_129038(manager_mutex)).unwrap();
    // spawner.spawn(task_ais_class_b_129039(manager_mutex)).unwrap();
    spawner.spawn(task_navigation_129284(manager_mutex)).unwrap();

    defmt::info!("Tâches lancées, main loop en attente...");

    // Boucle principale qui rend la main à l'executor
    loop {
        embassy_time::Timer::after(Duration::from_secs(10)).await;
        defmt::info!("Main loop still alive");
    }
}

#[embassy_executor::task]
async fn test_ticker() {
    let mut ticker = Ticker::every(Duration::from_millis(10000));
    let mut i = 0;
    loop {
        i += 1;
        ticker.next().await;
        println!("Tick #{} après 10s", i);
    }
}
