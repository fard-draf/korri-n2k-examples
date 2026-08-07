use super::Handle;
use shared_core::{publisher::publish, samples::AlertText};

#[embassy_executor::task]
pub async fn task_alert_text_126985(handle: Handle) {
    publish(handle, AlertText::new()).await;
}
