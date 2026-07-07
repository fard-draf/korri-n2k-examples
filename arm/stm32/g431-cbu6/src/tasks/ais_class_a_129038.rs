use super::Handle;

#[embassy_executor::task]
pub async fn task_ais_class_a_129038(handle: Handle) {
    shared_core::pgns::ais_class_a_129038::task_ais_class_a_129038(handle).await;
}
