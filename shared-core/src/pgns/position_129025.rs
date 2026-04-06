use korri_n2k::protocol::transport::traits::can_bus::CanBus;
use korri_n2k::protocol::transport::traits::korri_timer::KorriTimer;
use defmt::{info, Debug2Format};
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{managment::address_manager::AddressManager, messages::Pgn129025};

pub async fn task_position_129025<C, T>(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManager<C, T>>,
)
where
    C: CanBus + Send + 'static,
    T: KorriTimer + Send + 'static,
    C::Error: core::fmt::Debug,
{
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        ticker.next().await;
        let mut position_pgn = Pgn129025::new();
        position_pgn.latitude = 47.64425;
        position_pgn.longitude = -2.71842;

        let mut mgr = manager.lock().await;
        match mgr.send_pgn(&position_pgn, 129025, None).await {


            Ok(_) => {
                info!("PGN 129284 sent successfully");
            }
            Err(e) => {
                info!("Error sending PGN 129284: {:?}", Debug2Format(&e));
            }
        }
    }
}
