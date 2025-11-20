use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use embassy_time::Duration;
use korri_n2k::protocol::transport::traits::korri_timer::KorriTimer;

// Même wrapper que pour EspCanBus - rend les futures Send sur ESP32
struct SendFuture<F> {
    inner: F,
}

impl<F> SendFuture<F> {
    fn new(inner: F) -> Self {
        Self { inner }
    }
}

// SAFETY: Sur ESP32, tout s'exécute sur un seul thread avec embassy-executor
unsafe impl<F> Send for SendFuture<F> {}

impl<F: Future> Future for SendFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            let this = self.get_unchecked_mut();
            Pin::new_unchecked(&mut this.inner).poll(cx)
        }
    }
}

pub struct EspTimer;

impl KorriTimer for EspTimer {
    fn delay_ms<'a>(&'a mut self, millis: u32) -> impl Future<Output = ()> + 'a {
        SendFuture::new(async move {
            embassy_time::Timer::after(Duration::from_millis(millis as u64)).await
        })
    }
}
