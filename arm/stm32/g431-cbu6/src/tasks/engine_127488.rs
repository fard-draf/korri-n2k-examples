use super::Handle;
use shared_core::{publisher::publish, samples::EngineRapid};

#[embassy_executor::task]
pub async fn task_engine_127488(handle: Handle) {
    publish(handle, EngineRapid::new()).await;
}
