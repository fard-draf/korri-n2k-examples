use super::Handle;
use shared_core::{publisher::publish, samples::Depth};

#[embassy_executor::task]
pub async fn task_depth_128267(handle: Handle) {
    publish(handle, Depth::new()).await;
}
