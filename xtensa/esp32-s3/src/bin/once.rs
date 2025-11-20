#![no_std]
#![no_main]

use core::{any::Any, fmt::Debug};
use esp_backtrace as _;
use esp_println as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer, Ticker};
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::{Io, Level, Output, OutputConfig},
    main, peripherals, system,
    time::Instant,
    timer::timg::TimerGroup,
    twai::{
        self, BaudRate, EspTwaiFrame, ExtendedId, StandardId, Twai, TwaiConfiguration, TwaiMode,
    },
    Blocking,
};
use esp_hal_embassy::init;

use korri_n2k::{
    infra::codec::traits::{PgnData, ToPayload},
    protocol::{
        lookups::SystemTime,
        managment::address_claiming::claim_address,
        messages::{Pgn129025, Pgn59904},
        transport::{
            can_frame::CanFrame,
            can_id::{self, CanId},
            traits::{can_bus::CanBus, korri_timer::KorriTimer},
        },
    },
};
use nb::block;

esp_bootloader_esp_idf::esp_app_desc!();
// You need a panic handler. Usually, you you would use esp_backtrace, panic-probe, or
// something similar, but you can also bring your own like this:
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    esp_hal::system::software_reset()
}

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    defmt::println!("TRANSMITER - Init async..");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let mut led = Output::new(peripherals.GPIO2, Level::Low, OutputConfig::default());

    let can_tx_pin = peripherals.GPIO42; //violet
    let can_rx_pin = peripherals.GPIO41; //orange

    const TWAI_BAUDRATE: twai::BaudRate = BaudRate::B250K;

    let mut can_config = twai::TwaiConfiguration::new(
        peripherals.TWAI0,
        can_rx_pin,
        can_tx_pin,
        TWAI_BAUDRATE,
        TwaiMode::Normal,
    )
    .into_async();

    let mut can = can_config.start();

    defmt::println!("TWAI async démarré");

    led.set_high();
    Timer::after(Duration::from_millis(1000)).await;
    led.set_low();

    let emitter_address = 42;
    let mut buffer = [0u8; 8];
    let mut ticker = Ticker::every(Duration::from_millis(1500));

    loop {
        let mut position_pgn = Pgn129025::new();
        position_pgn.latitude = 47.64425;
        position_pgn.longitude = -2.71842;

        let can_id = CanId::builder(129025, emitter_address)
            .with_priority(2)
            .build()
            .expect("La construction doit reussir");

        let payload_len = position_pgn
            .to_payload(&mut buffer)
            .expect("Serialisation failed");

        let frame = CanFrame {
            id: can_id,
            data: buffer,
            len: payload_len,
        };
        // Construire directement le TwaiFrame
        let ext_id = ExtendedId::new(frame.id.0).unwrap();

        let twai_frame =
            EspTwaiFrame::new(ext_id, &buffer[..payload_len]).expect("Frame creation failed");

        if let Err(e) = can.transmit_async(&twai_frame).await {
            defmt::println!("TX error: {:?}", e);
        }
        ticker.next().await;
    }
}
