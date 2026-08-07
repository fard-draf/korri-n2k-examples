use super::Handle;
use shared_core::{publisher::publish, samples::AcInput};

#[embassy_executor::task]
pub async fn task_ac_input_127503(handle: Handle) {
    publish(handle, AcInput::new()).await;
}
