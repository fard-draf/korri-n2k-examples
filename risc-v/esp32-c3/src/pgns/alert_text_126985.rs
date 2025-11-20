use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        lookups::{AlertCategory, AlertLanguageId, AlertType},
        managment::address_manager::AddressManager,
        messages::Pgn126985,
        transport::fast_packet::builder::FastPacketBuilder,
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_alert_text_126985(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let interval = Pgn126985::PGN_126985_DESCRIPTOR
        .trans_interval
        .unwrap_or(60000) as u64;

    let priority = Pgn126985::PGN_126985_DESCRIPTOR
        .priority
        .unwrap_or(3);

    let mut ticker = Ticker::every(Duration::from_millis(interval));
    let mut payload_buffer = [0u8; 200];

    loop {
        ticker.next().await;

        let mut alert_pgn = Pgn126985::new();
        alert_pgn.alert_type = AlertType::Alarm;
        alert_pgn.alert_category = AlertCategory::Technical;
        alert_pgn.alert_system = 1;
        alert_pgn.alert_sub_system = 0;
        alert_pgn.alert_id = 100;
        alert_pgn.language_id = AlertLanguageId::EnglishUs;

        let payload_len = match alert_pgn.to_payload(&mut payload_buffer) {
            Ok(len) => len,
            Err(_) => continue,
        };

        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let fp_builder = FastPacketBuilder::new(126985, my_address, None, &payload_buffer[..payload_len]);

        {
            let mut mgr = manager.lock().await;
            for frame_result in fp_builder.build() {
                match frame_result {
                    Ok(frame) => {
                        if let Err(_e) = mgr.send(&frame).await {
                            // Gestion d'erreur simplifiée
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
