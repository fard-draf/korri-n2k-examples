use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use crate::{ports::EspCanBus, timer::EspTimer};
use korri_n2k::{
    infra::codec::traits::{PgnData, ToPayload},
    protocol::{
        lookups::{ControllerState, EquipmentStatus, PgnErrorCode, WaterReference},
        managment::address_manager::AddressManager,
        messages::Pgn126993,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
            traits::{can_bus, korri_timer::KorriTimer, pgn_sender::PgnSender},
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_heartbeat_126993(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let interval = Pgn126993::PGN_126993_DESCRIPTOR
        .trans_interval
        .unwrap_or(60000) as u64;

    let priority = Pgn126993::PGN_126993_DESCRIPTOR.priority;
    let mut ticker = Ticker::every(Duration::from_millis(interval));
    let mut buffer = [0u8; 8];

    loop {
        ticker.next().await;
        let mut heartbeat_pgn = Pgn126993::new();
        heartbeat_pgn.equipment_status = EquipmentStatus::Operational;
        heartbeat_pgn.controller1_state = ControllerState::ErrorPassive;
        heartbeat_pgn.data_transmit_offset = 0.0;

        let payload_len = heartbeat_pgn
            .to_payload(&mut buffer)
            .expect("Serialisation failed");

        // Verrouiller le mutex pour accéder au manager
        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let can_id = CanId::builder(126993, my_address)
            .with_priority(priority.unwrap_or(3))
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
