use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::{PgnData, ToPayload},
    protocol::{
        managment::address_manager::AddressManager,
        messages::Pgn129025,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_position_129025(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut buffer = [0u8; 8];

    loop {
        ticker.next().await;
        let mut position_pgn = Pgn129025::new();
        position_pgn.latitude = 47.64425;
        position_pgn.longitude = -2.71842;

        let payload_len = position_pgn
            .to_payload(&mut buffer)
            .expect("Serialisation failed");

        // Verrouiller le mutex pour accéder au manager
        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let can_id = CanId::builder(129025, my_address)
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
