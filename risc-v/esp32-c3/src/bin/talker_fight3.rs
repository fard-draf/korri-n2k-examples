#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_println as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker,Timer};
use esp_hal::{
    gpio::{Level, Output, OutputConfig},
    clock::CpuClock,
    timer::timg::TimerGroup,
    twai::{BaudRate, TwaiMode},
};
use esp32_c3::{
    manager_service::{address_manager_task, init_manager},
    pgns::{depth_128267::task_depth_128267, speed_128259::task_speed_128259},
};
use korri_n2k::protocol::managment::address_manager::AddressManager;

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

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    defmt::println!("TRANSMITER - Init async..");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    // let mut led = Output::new(peripherals.GPIO1, Level::Low, OutputConfig::default());

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let can_tx_pin = peripherals.GPIO9; //vert
    let can_rx_pin = peripherals.GPIO10; //bleu

    let can_config = esp_hal::twai::TwaiConfiguration::new(
        peripherals.TWAI0,
        can_rx_pin,
        can_tx_pin,
        BaudRate::B250K,
        TwaiMode::Normal,
    )
    .into_async();

    let can_peripheral = can_config.start();
    let can_bus = esp32_c3::ports::EspCanBus::new(can_peripheral);

    let korri_timer = esp32_c3::timer::EspTimer;



    let my_name = 0xFFD5967890ABCFFF;
    let preferred_address = 148;

    defmt::info!("Creating AddressManager...");

    // Créer l'AddressManager (fait le claim iniial automatiquement)
    let mut manager = match AddressManager::new(can_bus, korri_timer, my_name, preferred_address).await
    {
        Ok(mgr) => mgr,
        Err(_) => panic!("Failed to create AddressManager"),
    };

    let claimed_address = manager.current_address();
    defmt::info!("Address claimed: {}", claimed_address);
    let (runner, manager_handle) = init_manager(manager);
    spawner.spawn(address_manager_task(runner)).unwrap();
    // spawner.spawn(led_ticker(led)).unwrap();
    // spawner.spawn(test_ticker()).unwrap();
    // spawner.spawn(task_position_129025(manager_handle)).unwrap();
    spawner
        .spawn(task_depth_128267(manager_handle))
        .unwrap();
    // spawner.spawn(task_heartbeat_126993(manager_mutex)).unwrap();
    // spawner
    //     .spawn(task_speed_128259(manager_handle))
    //     .unwrap();
    // spawner.spawn(task_engine_127488(manager_handle)).unwrap();
    // spawner.spawn(task_engine_127489(manager_handle)).unwrap();
    // spawner.spawn(task_alert_text_126985(manager_handle)).unwrap();
    // spawner.spawn(task_heading_control_127237(manager_handle)).unwrap();
    // spawner.spawn(task_rudder_127245(manager_handle)).unwrap();
    // spawner.spawn(task_ac_input_127503(manager_handle)).unwrap();
    // spawner.spawn(task_ais_class_a_129038(manager_handle)).unwrap();
    // spawner.spawn(task_ais_class_b_129039(manager_handle)).unwrap();
    // spawner.spawn(task_datum_129044(manager_handle)).unwrap();
    // spawner.spawn(task_navigation_129284(manager_handle)).unwrap();
    // spawner.spawn(task_environmental_130310(manager_mutex)).unwrap();


    // Boucle principale qui rend la main à l'executor
    loop {
        embassy_time::Timer::after(Duration::from_secs(10)).await;
        defmt::info!("Main loop still alive");
    }
}


// #[embassy_executor::task]
// async fn led_ticker(mut led: Output<'static>) {
//     led.toggle();
//     let mut ticker = Ticker::every(Duration::from_millis(1000));
//     ticker.next().await;
//     led.toggle();
    
        
// }


#[embassy_executor::task(pool_size = 1)]
async fn test_ticker() {
    let mut ticker = Ticker::every(Duration::from_millis(10000));
    let mut i = 0;
    loop {
        i += 1;
        ticker.next().await;
        defmt::println!("Tick #{} after 10sec", i);
    }
}
