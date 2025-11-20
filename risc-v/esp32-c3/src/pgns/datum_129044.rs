use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        managment::address_manager::AddressManager,
        messages::Pgn129044,
        transport::fast_packet::builder::FastPacketBuilder,
    },
};
use esp_println::println;

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_datum_129044(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let interval = Pgn129044::PGN_129044_DESCRIPTOR
        .trans_interval
        .unwrap_or(10010) as u64;

    let priority = Pgn129044::PGN_129044_DESCRIPTOR
        .priority
        .unwrap_or(6);

    let mut ticker = Ticker::every(Duration::from_millis(interval));
    let mut payload_buffer = [0u8; 256];

    loop {
        ticker.next().await;

        let mut datum_pgn = Pgn129044::new();
        // WGS84 datum
        datum_pgn.local_datum = [b'W', b'G', b'S', b'8'];
        datum_pgn.delta_latitude = 47.0;
        datum_pgn.delta_longitude = -2.0;
        datum_pgn.delta_altitude = 16.0;
        datum_pgn.reference_datum = [b'W', b'G', b'S', b'8'];

        let payload_len = match datum_pgn.to_payload(&mut payload_buffer) {
            Ok(len) => len,
            Err(_) => continue,
        };

        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let fp_builder = FastPacketBuilder::new(129044, my_address, None, &payload_buffer[..payload_len]);

        {
            let mut mgr = manager.lock().await;
            for frame_result in fp_builder.build() {
                match frame_result {
                    Ok(frame) => {
                        if let Err(_e) = mgr.send(&frame).await {
                            // Gestion d'erreur simplifiée
                            println!("Fast Packet Datum error");
                        }
                    }
                    Err(_e) => {
                        break;
                    }
                }
            }
        }
    }
}
