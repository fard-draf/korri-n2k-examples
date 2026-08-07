use super::Handle;
use shared_core::{publisher::publish, samples::HeadingControl};

#[embassy_executor::task]
pub async fn task_heading_control_127237(handle: Handle) {
    publish(handle, HeadingControl::new()).await;
}
