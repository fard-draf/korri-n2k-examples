use super::Handle;

#[embassy_executor::task]
pub async fn task_alert_text_126985(handle: Handle) {
    shared_core::pgns::alert_text_126985::task_alert_text_126985(handle).await;
}
