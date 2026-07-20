use super::Handle;

#[embassy_executor::task]
pub async fn task_environmental_130310(handle: Handle) {
    shared_core::pgns::environmental_130310::task_environmental_130310(handle).await;
}
