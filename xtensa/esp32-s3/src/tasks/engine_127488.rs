use super::Handle;

#[embassy_executor::task]
pub async fn task_engine_127488(handle: Handle) {
    shared_core::pgns::engine_127488::task_engine_127488(handle).await;
}
