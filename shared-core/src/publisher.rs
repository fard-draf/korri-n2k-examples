//! The loop that turns a [`Sample`](crate::samples::Sample) into bus traffic.
//!
//! One version per runtime, because the two `AddressHandle` types do not match:
//! embassy carries the channel capacity in a const generic, tokio does not.
//! Everything above this file stays runtime free.

#[cfg(feature = "embassy")]
pub use embassy::publish;
#[cfg(feature = "tokio")]
pub use tokio_rt::publish;

#[cfg(feature = "embassy")]
mod embassy {
    use crate::samples::Sample;
    use defmt::{Debug2Format, trace, warn};
    use embassy_time::{Duration, Ticker};
    use korri_n2k::protocol::management::address_supervisor::AddressHandle;

    /// Publish `sample` forever, once per `PERIOD_MS`.
    ///
    /// Publishing pauses while no address is held. A refusal racing a later
    /// address loss is logged and the loop carries on.
    pub async fn publish<S: Sample, const CAP: usize>(
        handle: &'static AddressHandle<'static, CAP>,
        mut sample: S,
    ) {
        let mut ticker = Ticker::every(Duration::from_millis(S::PERIOD_MS));

        loop {
            ticker.next().await;

            let Some(address) = handle.claimed_address() else {
                continue;
            };

            let data = sample.next();
            match handle.send_pgn(&data, S::PGN, S::PRIORITY, None).await {
                Ok(()) => trace!(
                    "Parameter Group Number {=u32} queued from claimed address {=u8}",
                    S::PGN,
                    address
                ),
                Err(error) => warn!(
                    "Parameter Group Number {=u32} not queued: {}",
                    S::PGN,
                    Debug2Format(&error)
                ),
            }
        }
    }
}

#[cfg(feature = "tokio")]
mod tokio_rt {
    use crate::samples::Sample;
    use korri_n2k::protocol::management::address_supervisor::AddressHandle;
    use std::sync::Arc;

    /// Publish `sample` forever, once per `PERIOD_MS`.
    ///
    /// `Arc` because one handle feeds several publishing tasks and the tokio
    /// `AddressHandle` is not `Clone`, though both its fields are. Embassy needs
    /// no wrapper: its handle lives in a `static`.
    ///
    /// Returns when the runner is gone: nothing would execute the command any
    /// more, so spinning on it would only print.
    pub async fn publish<S: Sample>(handle: Arc<AddressHandle>, mut sample: S) {
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(S::PERIOD_MS));

        loop {
            ticker.tick().await;

            let data = sample.next();
            match handle.send_pgn(&data, S::PGN, S::PRIORITY, None).await {
                Ok(()) => println!("PGN {} queued from {:?}", S::PGN, handle.claimed_address()),
                Err(error) => {
                    eprintln!("PGN {} not queued: {error:?}", S::PGN);
                    return;
                }
            }
        }
    }
}
