use super::Handle;

#[embassy_executor::task]
pub async fn task_ac_input_127503(handle: Handle) {
    shared_core::pgns::ac_input_127503::task_ac_input_127503(handle).await;
}
