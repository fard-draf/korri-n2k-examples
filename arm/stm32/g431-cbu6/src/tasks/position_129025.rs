use super::Handle;

#[embassy_executor::task]
pub async fn task_position_129025(handle: Handle) {
    shared_core::pgns::position_129025::task_position_129025(handle).await;
}
