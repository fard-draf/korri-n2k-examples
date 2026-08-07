use super::Handle;
use shared_core::{publisher::publish, samples::EngineDynamic};

#[embassy_executor::task]
pub async fn task_engine_127489(handle: Handle) {
    publish(handle, EngineDynamic::new()).await;
}
