use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use defmt::{info, Debug2Format};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        lookups::{AlertCategory, AlertLanguageId, AlertType},
        managment::address_manager::AddressManager,
        messages::Pgn126985,
        transport::{
            fast_packet::builder::FastPacketBuilder,
            traits::pgn_sender::PgnSender,
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_alert_text_126985(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {

    let mut ticker = Ticker::every(Duration::from_millis(1000));

    loop {
        ticker.next().await;

        let mut alert_pgn = Pgn126985::new();
        alert_pgn.alert_type = AlertType::Alarm;
        alert_pgn.alert_category = AlertCategory::Technical;
        alert_pgn.alert_system = 1;
        alert_pgn.alert_sub_system = 0;
        alert_pgn.alert_id = 100;
        alert_pgn.language_id = AlertLanguageId::EnglishUs;

        {
            let mut mgr = manager.lock().await;
            match mgr.send_pgn(&alert_pgn, 126985, None).await {
                Ok(_) => {
                    info!("PGN 127503 sent successfully");
                }
                Err(e) => {
                    info!("Error sending PGN 127503: {:?}", Debug2Format(&e));
                }
            
            }
        }
    }
}
