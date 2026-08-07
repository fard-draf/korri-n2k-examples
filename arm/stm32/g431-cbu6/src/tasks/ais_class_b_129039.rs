use super::Handle;
use shared_core::{publisher::publish, samples::AisClassB};

#[embassy_executor::task]
pub async fn task_ais_class_b_129039(handle: Handle) {
    publish(handle, AisClassB::new()).await;
}
