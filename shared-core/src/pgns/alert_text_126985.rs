use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};
use defmt::{info, Debug2Format};

use korri_n2k::protocol::{
        lookups::{AlertCategory, AlertLanguageId, AlertType},
        managment::address_manager::AddressManager,
        messages::Pgn126985 };


pub async fn task_alert_text_126985<const CAP: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, CAP>,
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
                        match handle.send_pgn(&alert_pgn, 126985, 6, None).await {
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
