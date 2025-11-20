use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        managment::address_manager::AddressManager,
        messages::Pgn130310,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_environmental_130310(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let interval = Pgn130310::PGN_130310_DESCRIPTOR
        .trans_interval
        .unwrap_or(500) as u64;

    let priority = Pgn130310::PGN_130310_DESCRIPTOR
        .priority
        .unwrap_or(5);

    let mut ticker = Ticker::every(Duration::from_millis(interval));
    let mut buffer = [0u8; 8];
    let mut water_temp: f32 = 18.0;

    loop {
        ticker.next().await;

        let mut env_pgn = Pgn130310::new();
        env_pgn.sid = 1;
        env_pgn.water_temperature = water_temp;
        env_pgn.outside_ambient_air_temperature = 22.0;
        env_pgn.atmospheric_pressure = 101325.0;

        water_temp = 18.0 + (water_temp - 18.0 + 0.1) % 5.0;

        let payload_len = match env_pgn.to_payload(&mut buffer) {
            Ok(len) => len,
            Err(_) => continue,
        };

        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let can_id = CanId::builder(130310, my_address)
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
