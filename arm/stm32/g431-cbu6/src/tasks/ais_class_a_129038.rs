use super::Handle;
use shared_core::{publisher::publish, samples::AisClassA};

#[embassy_executor::task]
pub async fn task_ais_class_a_129038(handle: Handle) {
    publish(handle, AisClassA::new()).await;
}
