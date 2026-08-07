use super::Handle;
use shared_core::{publisher::publish, samples::Datum};

#[embassy_executor::task]
pub async fn task_datum_129044(handle: Handle) {
    publish(handle, Datum::new()).await;
}
