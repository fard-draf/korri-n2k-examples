use korri_n2k::protocol::transport::traits::can_bus::CanBus;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        lookups::EngineInstance,
        managment::address_manager::AddressManager,
        messages::Pgn127488,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
            traits::korri_timer::KorriTimer,
        },
    },
};


pub async fn task_engine_127488<C, T>(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManager<C, T>>,
)
where
    C: CanBus + Send + 'static,
    T: KorriTimer + Send + 'static,
    C::Error: core::fmt::Debug,
{
    let mut ticker = Ticker::every(Duration::from_millis(100));
    let mut buffer = [0u8; 8];

    let mut tilt_trim: i8 = 0;
    let mut rpm: u16 = 0;

    loop {
        ticker.next().await;
        let mut engine_pgn = Pgn127488::new();
        engine_pgn.instance = EngineInstance::SingleEngineOrDualEnginePort;
        engine_pgn.speed = (1000.0 + (rpm as f32)) % 2500.0;
        engine_pgn.boost_pressure = 1478.0;
        engine_pgn.tilt_trim = tilt_trim;

        rpm = rpm.wrapping_add(1);
        tilt_trim = (tilt_trim + 1) % 101;

        let payload_len = engine_pgn
            .to_payload(&mut buffer)
            .expect("Serialisation failed");

        // Verrouiller le mutex pour accéder au manager
        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let can_id = CanId::builder(127488, my_address)
            .with_priority(2)
            .build()
            .expect("La construction doit reussir");

        let frame = CanFrame {
            id: can_id,
            data: buffer,
            len: payload_len,
        };

        // Verrouiller à nouveau pour envoyer
        let result = {
            let mut mgr = manager.lock().await;
            mgr.send(&frame).await
        };
    }
}
