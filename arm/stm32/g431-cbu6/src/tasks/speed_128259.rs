use super::Handle;

#[embassy_executor::task]
pub async fn task_speed_128259(handle: Handle) {
    shared_core::pgns::speed_128259::task_speed_128259(handle).await;
}
