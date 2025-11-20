use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use crate::{ports::EspCanBus, timer::EspTimer};
use korri_n2k::{
    infra::codec::traits::{PgnData, ToPayload},
    protocol::{
        lookups::{ControllerState, EquipmentStatus, PgnErrorCode, WaterReference},
        managment::address_manager::AddressManager,
        messages::Pgn128267,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
            traits::{can_bus, korri_timer::KorriTimer, pgn_sender::PgnSender},
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_depth_128267(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(2));
    let mut buffer = [0u8; 8];

    let mut sim_depth: u16 = 1;

    loop {
        ticker.next().await;
        let mut depth_pgn = Pgn128267::new();
        depth_pgn.sid = 42;
        depth_pgn.depth = (25.0 + ((sim_depth) as f32)) % 255.0;
        depth_pgn.offset = 0.0;
        depth_pgn.range = 50.0;

        sim_depth = sim_depth.wrapping_add(1);

        let payload_len = depth_pgn
            .to_payload(&mut buffer)
            .expect("Serialisation failed");

        // Verrouiller le mutex pour accéder au manager
        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let can_id = CanId::builder(128267, my_address)
            .with_priority(3)
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
