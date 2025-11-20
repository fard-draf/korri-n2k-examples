use embassy_time::{Duration, Ticker};

use crate::manager_service::{ManagerClientError, ManagerHandle};
use korri_n2k::protocol::messages::Pgn128267;

#[embassy_executor::task]
pub async fn task_depth_128267(
    manager: &'static ManagerHandle,
) {
    let mut ticker = Ticker::every(Duration::from_secs(2));

    let mut sim_depth: u16 = 1;

    loop {
        ticker.next().await;
        let mut depth_pgn = Pgn128267::new();
        depth_pgn.sid = 42;
        depth_pgn.depth = (25.0 + ((sim_depth) as f32)) % 255.0;
        depth_pgn.offset = 0.0;
        depth_pgn.range = 50.0;

        sim_depth = sim_depth.wrapping_add(1);

        if let Err(err) = manager
            .send_pgn(&depth_pgn, 128267, 3, None)
            .await
        {
            if !matches!(err, ManagerClientError::Serialization) {
                defmt::warn!("Failed to enqueue PGN 128267");
            }
        }
    }
}
