use embassy_time::{Duration, Ticker};

use crate::manager_service::{ManagerClientError, ManagerHandle};
use korri_n2k::protocol::{
    lookups::WaterReference,
    messages::Pgn128259,
};

#[embassy_executor::task]
pub async fn task_speed_128259(
    manager: &'static ManagerHandle,
) {
    defmt::info!("task_speed démarrée");
    let interval = Pgn128259::PGN_128259_DESCRIPTOR
        .trans_interval
        .unwrap_or(5000) as u64;
    let priority = Pgn128259::PGN_128259_DESCRIPTOR.priority.unwrap_or(3);
    let mut ticker = Ticker::every(Duration::from_millis(interval));

    loop {
        ticker.next().await;
        let mut speed = Pgn128259::new();
        speed.sid = 240;
        speed.speed_water_referenced = 5.5;
        speed.speed_ground_referenced = 55.8;
        speed.speed_water_referenced_type = WaterReference::PaddleWheel;
        speed.speed_direction = 158;

        if let Err(err) = manager
            .send_pgn(&speed, 128259, priority, None)
            .await
        {
            if !matches!(err, ManagerClientError::Serialization) {
                defmt::warn!("Failed to enqueue PGN 128259");
            }
        }
    }
}
