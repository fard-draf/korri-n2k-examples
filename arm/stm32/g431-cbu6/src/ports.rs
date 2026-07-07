use defmt::trace;
use embassy_stm32::can::{
    self, Frame as HalFrame,
    enums::{BusError, FrameCreateError},
};
use embassy_time::{Duration, Timer};
use embedded_can::Id;
use korri_n2k::protocol::transport::{
    can_frame::CanFrame,
    can_id::CanId,
    traits::{can_bus::CanBus, korri_timer::KorriTimer},
};

/// Async CAN adapter that bridges the Embassy FDCAN driver with the `korri-n2k`
/// transport traits.
pub struct Stm32CanBus<'d, const TX_BUF: usize, const RX_BUF: usize> {
    inner: can::BufferedCan<'d, TX_BUF, RX_BUF>,
}

impl<'d, const TX_BUF: usize, const RX_BUF: usize> Stm32CanBus<'d, TX_BUF, RX_BUF> {
    pub fn new(inner: can::BufferedCan<'d, TX_BUF, RX_BUF>) -> Self {
        Self { inner }
    }
}

#[derive(Debug)]
pub enum Stm32CanError {
    Bus(BusError),
    FrameCreate(FrameCreateError),
    NonExtendedId,
}

impl From<BusError> for Stm32CanError {
    fn from(value: BusError) -> Self {
        Self::Bus(value)
    }
}

impl From<FrameCreateError> for Stm32CanError {
    fn from(value: FrameCreateError) -> Self {
        Self::FrameCreate(value)
    }
}

impl defmt::Format for Stm32CanError {
    fn format(&self, fmt: defmt::Formatter<'_>) {
        match self {
            Stm32CanError::Bus(err) => defmt::write!(
                fmt,
                "Bus({})",
                match err {
                    BusError::Stuff => "stuff",
                    BusError::Form => "form",
                    BusError::Acknowledge => "ack",
                    BusError::BitRecessive => "bit_recessive",
                    BusError::BitDominant => "bit_dominant",
                    BusError::Crc => "crc",
                    BusError::Software => "software",
                }
            ),
            Stm32CanError::FrameCreate(err) => defmt::write!(
                fmt,
                "Frame({})",
                match err {
                    FrameCreateError::NotEnoughData => "not_enough_data",
                    FrameCreateError::InvalidDataLength => "invalid_data_length",
                    FrameCreateError::InvalidCanId => "invalid_can_id",
                }
            ),
            Stm32CanError::NonExtendedId => defmt::write!(fmt, "NonExtendedId"),
        }
    }
}

impl<'d, const TX_BUF: usize, const RX_BUF: usize> CanBus for Stm32CanBus<'d, TX_BUF, RX_BUF> {
    type Error = Stm32CanError;

    async fn send(&mut self, frame: &CanFrame) -> Result<(), Self::Error> {
        let hal_frame = HalFrame::new_extended(frame.id.0, &frame.data[..frame.len])
            .map_err(|err| Self::Error::from(err))?;
        trace!("HAL TX id=0x{=u32:X} len={}", frame.id.0, frame.len);
        self.inner.write(hal_frame).await;
        Ok(())
    }

    async fn recv(&mut self) -> Result<CanFrame, Self::Error> {
        loop {
            match self.inner.read().await {
                Ok(envelope) => {
                    let hal_frame = envelope.frame;
                    let raw_id = match hal_frame.id() {
                        Id::Extended(id) => id.as_raw(),
                        Id::Standard(_) => {
                            continue; // non-n2k frame, skip silently
                        }
                    };
                    let payload = hal_frame.data();
                    let mut data = [0u8; 8];
                    data[..payload.len()].copy_from_slice(payload);
                    return Ok(CanFrame {
                        id: CanId(raw_id),
                        data,
                        len: payload.len(),
                    });
                }
                Err(err) => {
                    // BusOff: the FDCAN controller disabled itself — unrecoverable.
                    // All other errors (Warning, Passive, Stuff, Form, Ack, Crc…)
                    // are transient; retry rather than killing the claim or runner.
                    defmt::warn!("CAN rx soft error (ignored): {}", Stm32CanError::Bus(err));
                }
            }
        }
    }
}

/// Embassy timer adapter that fulfils `KorriTimer`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stm32Timer;

impl Stm32Timer {
    pub const fn new() -> Self {
        Self
    }
}

impl KorriTimer for Stm32Timer {
    async fn delay_ms(&mut self, millis: u32) -> () {
        async move {
            Timer::after(Duration::from_millis(millis as u64)).await;
        }
        .await
    }
}
