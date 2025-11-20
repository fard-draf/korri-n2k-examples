#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_println as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    timer::timg::TimerGroup,
    twai::{BaudRate, TwaiMode},
};
use korri_bsp_esp32::{
    manager_service::{address_manager_task, init_manager},
    pgns::{
        navigation_129284::task_navigation_129284, position_129025::task_position_129025,
    },
};
use korri_n2k::{
    protocol::managment::address_manager::AddressManager,
};

esp_bootloader_esp_idf::esp_app_desc!();


#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use esp_println::println;
    defmt::println!("PANIC OCCURRED: {:?}", info);

    loop {
        for _ in 0..10_000_000 {
            core::sync::atomic::spin_loop_hint();
        }
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    defmt::info!("=== TALKER START ===");
    defmt::println!("TRANSMITER - Init async..");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let mut led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

    let can_tx_pin = peripherals.GPIO42; //violet
    let can_rx_pin = peripherals.GPIO41; //orange

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

    let my_name = 0xFFD4567890ABCFFF;
    let preferred_address = 142;

    defmt::info!("Creating AddressManager...");

    // Créer l'AddressManager (fait le claim initial automatiquement)
    let mut manager =
        match AddressManager::new(can_bus, korri_timer, my_name, preferred_address).await {
            Ok(mgr) => mgr,
            Err(e) => panic!("Failed to create AddressManager: {:?}", e),
        };

    let claimed_address = manager.current_address();
    defmt::info!("Address claimed: {}", claimed_address);
    let (runner, manager_handle) = init_manager(manager);

    spawner
        .spawn(address_manager_task(runner))
        .unwrap();
    spawner.spawn(test_ticker()).unwrap();
    spawner
        .spawn(task_position_129025(manager_handle))
        .unwrap();
    spawner
        .spawn(task_navigation_129284(manager_handle))
        .unwrap();

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
        defmt::println!("Tick #{} après 10s", i);
    }
}
