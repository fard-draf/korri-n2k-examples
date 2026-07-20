use super::Handle;

#[embassy_executor::task]
pub async fn task_heartbeat_126993(handle: Handle) {
    shared_core::pgns::heartbeat_126993::task_heartbeat_126993(handle).await;
}
