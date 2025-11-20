use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use crate::{ports::EspCanBus, timer::EspTimer};
use korri_n2k::{
    infra::codec::traits::{PgnData, ToPayload},
    protocol::{
        lookups::{ControllerState, EquipmentStatus, PgnErrorCode, WaterReference},
        managment::address_manager::AddressManager,
        messages::Pgn128259,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
            traits::{can_bus, korri_timer::KorriTimer, pgn_sender::PgnSender},
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_speed_128259(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    defmt::info!("task_speed démarrée");
    let interval = Pgn128259::PGN_128259_DESCRIPTOR
        .trans_interval
        .unwrap_or(5000) as u64;
    let priority = Pgn128259::PGN_128259_DESCRIPTOR.priority.unwrap_or(3);
    let mut ticker = Ticker::every(Duration::from_millis(interval));
    let mut buffer = [0u8; 8];

    loop {
        ticker.next().await;
        let mut speed = Pgn128259::new();
        speed.sid = 240;
        speed.speed_water_referenced = 5.5;
        speed.speed_ground_referenced = 55.8;
        speed.speed_water_referenced_type = WaterReference::PaddleWheel;
        speed.speed_direction = 158;

        let payload_len = speed.to_payload(&mut buffer).expect("Serialisation failed");

        // Verrouiller le mutex pour accéder au manager
        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let can_id = CanId::builder(128259, my_address)
            .with_priority(priority)
            .build()
            .expect("La construction doit reussir");

        let frame = CanFrame {
            id: can_id,
            data: buffer,
            len: payload_len,
        };

        // Verrouiller à nouveau pour envoyer
        {
            let mut mgr = manager.lock().await;
            mgr.send(&frame).await
        };
    }
}
