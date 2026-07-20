use defmt::trace;
use embassy_time::{with_timeout, Duration, Timer};
use embedded_can::{Frame, Id};
use esp_hal::{
    twai::{EspTwaiError, EspTwaiFrame, ExtendedId as EspExtendedId, Twai},
    Async,
};
use korri_n2k::protocol::transport::{
    can_frame::CanFrame,
    can_id::CanId,
    traits::{can_bus::CanBus, korri_timer::KorriTimer},
    CAN_SEND_TIMEOUT_MS,
};

/// Async TWAI adapter that bridges the esp-hal controller with the `korri-n2k`
/// transport traits.
///
/// # ESP32 hardware limit
///
/// The TWAI controller only has a 3-frame TX buffer, and esp-hal has no
/// buffered driver like embassy-stm32's `BufferedCan`: `transmit_async` writes
/// straight to the hardware. Hence the explicit send timeout, without which a
/// saturated bus would block this task forever.
pub struct EspCanBus<'d> {
    can: Twai<'d, Async>,
}

impl<'d> EspCanBus<'d> {
    pub fn new(can: Twai<'d, Async>) -> Self {
        Self { can }
    }
}

#[derive(Debug)]
pub enum EspCanError {
    Twai(EspTwaiError),
    FrameCreate,
    SendTimeout,
}

impl From<EspTwaiError> for EspCanError {
    fn from(value: EspTwaiError) -> Self {
        Self::Twai(value)
    }
}

impl defmt::Format for EspCanError {
    fn format(&self, fmt: defmt::Formatter<'_>) {
        match self {
            EspCanError::Twai(err) => defmt::write!(fmt, "Twai({})", err),
            EspCanError::FrameCreate => defmt::write!(fmt, "FrameCreate"),
            EspCanError::SendTimeout => defmt::write!(fmt, "SendTimeout"),
        }
    }
}

impl<'d> CanBus for EspCanBus<'d> {
    type Error = EspCanError;

    async fn send(&mut self, frame: &CanFrame) -> Result<(), Self::Error> {
        let ext_id = EspExtendedId::new(frame.id.0).ok_or(EspCanError::FrameCreate)?;
        let twai_frame =
            EspTwaiFrame::new(ext_id, &frame.data[..frame.len]).ok_or(EspCanError::FrameCreate)?;

        trace!("HAL TX id=0x{=u32:X} len={}", frame.id.0, frame.len);

        // Only 3 frames fit in the hardware TX buffer.
        with_timeout(
            Duration::from_millis(CAN_SEND_TIMEOUT_MS as u64),
            self.can.transmit_async(&twai_frame),
        )
        .await
        .map_err(|_| EspCanError::SendTimeout)?
        .map_err(EspCanError::from)
    }

    async fn recv(&mut self) -> Result<CanFrame, Self::Error> {
        loop {
            match self.can.receive_async().await {
                Ok(twai_frame) => {
                    let raw_id = match twai_frame.id() {
                        Id::Extended(id) => id.as_raw(),
                        Id::Standard(_) => {
                            continue; // non-n2k frame, skip silently
                        }
                    };
                    let payload = twai_frame.data();
                    let mut data = [0u8; 8];
                    data[..payload.len()].copy_from_slice(payload);
                    return Ok(CanFrame {
                        id: CanId(raw_id),
                        data,
                        len: payload.len(),
                    });
                }
                Err(err) => {
                    // Transient errors: retry rather than kill the claim or
                    // the runner.
                    defmt::warn!("TWAI rx soft error (ignored): {}", EspCanError::Twai(err));
                }
            }
        }
    }
}

/// Embassy timer adapter implementing `KorriTimer`.
#[derive(Debug, Default, Clone, Copy)]
pub struct EspTimer;

impl EspTimer {
    pub const fn new() -> Self {
        Self
    }
}

impl KorriTimer for EspTimer {
    async fn delay_ms(&mut self, millis: u32) {
        Timer::after(Duration::from_millis(millis as u64)).await;
    }
}
