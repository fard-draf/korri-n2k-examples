use super::Handle;
use shared_core::{publisher::publish, samples::Environmental};

#[embassy_executor::task]
pub async fn task_environmental_130310(handle: Handle) {
    publish(handle, Environmental::new()).await;
}
