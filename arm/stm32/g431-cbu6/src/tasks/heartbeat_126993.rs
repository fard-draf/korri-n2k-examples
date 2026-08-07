use super::Handle;
use shared_core::{publisher::publish, samples::Heartbeat};

#[embassy_executor::task]
pub async fn task_heartbeat_126993(handle: Handle) {
    publish(handle, Heartbeat::new()).await;
}
