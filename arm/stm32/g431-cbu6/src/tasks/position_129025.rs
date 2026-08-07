use super::Handle;
use shared_core::{publisher::publish, samples::Position};

#[embassy_executor::task]
pub async fn task_position_129025(handle: Handle) {
    publish(handle, Position::new()).await;
}
