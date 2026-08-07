use super::Handle;
use shared_core::{publisher::publish, samples::Speed};

#[embassy_executor::task]
pub async fn task_speed_128259(handle: Handle) {
    publish(handle, Speed::new()).await;
}
