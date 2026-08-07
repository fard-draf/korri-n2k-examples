use super::Handle;
use shared_core::{publisher::publish, samples::Rudder};

#[embassy_executor::task]
pub async fn task_rudder_127245(handle: Handle) {
    publish(handle, Rudder::new()).await;
}
