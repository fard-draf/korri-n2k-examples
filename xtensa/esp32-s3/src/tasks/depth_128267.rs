use super::Handle;

#[embassy_executor::task]
pub async fn task_depth_128267(handle: Handle) {
    shared_core::pgns::depth_128267::task_depth_128267(handle).await;
}
