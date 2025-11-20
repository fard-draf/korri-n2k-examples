use embassy_time::{Duration, Ticker};

use crate::manager_service::{ManagerClientError, ManagerHandle};
use korri_n2k::protocol::messages::Pgn129025;

#[embassy_executor::task]
pub async fn task_position_129025(
    manager: &'static ManagerHandle,
) {
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        ticker.next().await;
        let mut position_pgn = Pgn129025::new();
        position_pgn.latitude = 47.64425;
        position_pgn.longitude = -2.71842;

        if let Err(err) = manager.send_pgn(&position_pgn, 129025, 2, None).await {
            if !matches!(err, ManagerClientError::Serialization) {
                defmt::warn!("Failed to enqueue PGN 129025");
            }
        }
    }
}
