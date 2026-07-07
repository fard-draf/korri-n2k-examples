use super::Handle;

#[embassy_executor::task]
pub async fn task_navigation_129284(handle: Handle) {
    shared_core::pgns::navigation_129284::task_navigation_129284(handle).await;
}
