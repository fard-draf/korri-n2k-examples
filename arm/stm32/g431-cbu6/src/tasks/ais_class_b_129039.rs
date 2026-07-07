use super::Handle;

#[embassy_executor::task]
pub async fn task_ais_class_b_129039(handle: Handle) {
    shared_core::pgns::ais_class_b_129039::task_ais_class_b_129039(handle).await;
}
