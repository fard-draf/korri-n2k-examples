use super::Handle;

#[embassy_executor::task]
pub async fn task_rudder_127245(handle: Handle) {
    shared_core::pgns::rudder_127245::task_rudder_127245(handle).await;
}
