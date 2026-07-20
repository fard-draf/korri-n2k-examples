use super::Handle;

#[embassy_executor::task]
pub async fn task_heading_control_127237(handle: Handle) {
    shared_core::pgns::heading_control_127237::task_heading_control_127237(handle).await;
}
