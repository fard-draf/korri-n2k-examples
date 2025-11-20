use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use crate::{ports::EspCanBus, timer::EspTimer};
use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        lookups::DirectionRudder,
        managment::address_manager::AddressManager,
        messages::Pgn127245,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
            traits::pgn_sender::PgnSender,
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_rudder_127245(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let interval = Pgn127245::PGN_127245_DESCRIPTOR
        .trans_interval
        .unwrap_or(100) as u64;

    let priority = Pgn127245::PGN_127245_DESCRIPTOR
        .priority
        .unwrap_or(2);

    let mut ticker = Ticker::every(Duration::from_millis(interval));
    let mut buffer = [0u8; 8];
    let mut position: f32 = 0.0;

    loop {
        ticker.next().await;

        let mut rudder_pgn = Pgn127245::new();
        rudder_pgn.instance = 0;
        rudder_pgn.direction_order = DirectionRudder::NoOrder;
        rudder_pgn.angle_order = 0.0;
        rudder_pgn.position = position;

        position = (position + 0.5) % 360.0;

        let payload_len = match rudder_pgn.to_payload(&mut buffer) {
            Ok(len) => len,
            Err(_) => continue,
        };

        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let can_id = CanId::builder(127245, my_address)
            .with_priority(priority)
            .build()
            .expect("La construction doit reussir");

        let frame = CanFrame {
            id: can_id,
            data: buffer,
            len: payload_len,
        };

        {
            let mut mgr = manager.lock().await;
            let _ = mgr.send(&frame).await;
        }
    }
}
