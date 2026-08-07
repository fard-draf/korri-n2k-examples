use super::Handle;
use shared_core::{publisher::publish, samples::Navigation};

#[embassy_executor::task]
pub async fn task_navigation_129284(handle: Handle) {
    publish(handle, Navigation::new()).await;
}
