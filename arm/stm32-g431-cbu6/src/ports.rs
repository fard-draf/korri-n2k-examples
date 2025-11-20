use core::future::Future;
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use defmt::trace;
use embassy_stm32::can::{
    self, Frame as HalFrame,
    enums::{BusError, BusErrorMode, FrameCreateError},
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

    fn refresh_diagnostics(&self) {
        record_bus_state(self.inner.properties());
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
                    BusError::BusOff => "bus_off",
                    BusError::BusPassive => "bus_passive",
                    BusError::BusWarning => "bus_warning",
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

    fn send<'a>(
        &'a mut self,
        frame: &'a CanFrame,
    ) -> impl Future<Output = Result<(), Self::Error>> + 'a {
        async move {
            let hal_frame =
                HalFrame::new_extended(frame.id.0, &frame.data[..frame.len]).map_err(|err| {
                    CAN_TX_FAILS.fetch_add(1, Ordering::Relaxed);
                    Self::Error::from(err)
                })?;
            trace!("HAL TX id=0x{=u32:X} len={}", frame.id.0, frame.len);
            self.inner.write(hal_frame).await;
            self.refresh_diagnostics();
            Ok(())
        }
    }

    fn recv<'a>(&'a mut self) -> impl Future<Output = Result<CanFrame, Self::Error>> + 'a {
        async move {
            let envelope = self.inner.read().await.map_err(|err| {
                CAN_RX_FAILS.fetch_add(1, Ordering::Relaxed);
                Self::Error::from(err)
            })?;
            let hal_frame = envelope.frame;

            let raw_id = match hal_frame.id() {
                Id::Extended(id) => id.as_raw(),
                Id::Standard(_) => {
                    CAN_RX_FAILS.fetch_add(1, Ordering::Relaxed);
                    return Err(Stm32CanError::NonExtendedId);
                }
            };

            let payload = hal_frame.data();
            let mut data = [0u8; 8];
            data[..payload.len()].copy_from_slice(payload);
            self.refresh_diagnostics();

            Ok(CanFrame {
                id: CanId(raw_id),
                data,
                len: payload.len(),
            })
        }
    }
}

static CAN_TX_FAILS: AtomicU32 = AtomicU32::new(0);
static CAN_RX_FAILS: AtomicU32 = AtomicU32::new(0);
static CAN_TX_ERR: AtomicU8 = AtomicU8::new(0);
static CAN_RX_ERR: AtomicU8 = AtomicU8::new(0);
static CAN_STATE: AtomicU8 = AtomicU8::new(BusState::ErrorActive as u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusState {
    ErrorActive = 0,
    ErrorPassive = 1,
    BusOff = 2,
}

impl BusState {
    fn from_mode(mode: BusErrorMode) -> Self {
        match mode {
            BusErrorMode::ErrorActive => BusState::ErrorActive,
            BusErrorMode::ErrorPassive => BusState::ErrorPassive,
            BusErrorMode::BusOff => BusState::BusOff,
        }
    }
}

impl defmt::Format for BusState {
    fn format(&self, fmt: defmt::Formatter<'_>) {
        match self {
            BusState::ErrorActive => defmt::write!(fmt, "ErrorActive"),
            BusState::ErrorPassive => defmt::write!(fmt, "ErrorPassive"),
            BusState::BusOff => defmt::write!(fmt, "BusOff"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CanDiagnostics {
    pub tx_err: u8,
    pub rx_err: u8,
    pub tx_failures: u32,
    pub rx_failures: u32,
    pub bus_state: BusState,
}

impl defmt::Format for CanDiagnostics {
    fn format(&self, fmt: defmt::Formatter<'_>) {
        defmt::write!(
            fmt,
            "diag(tx_err={}, rx_err={}, tx_fail={}, rx_fail={}, state={:?})",
            self.tx_err,
            self.rx_err,
            self.tx_failures,
            self.rx_failures,
            self.bus_state
        );
    }
}

fn record_bus_state(props: &can::Properties) {
    CAN_TX_ERR.store(props.tx_error_count(), Ordering::Relaxed);
    CAN_RX_ERR.store(props.rx_error_count(), Ordering::Relaxed);
    let state = BusState::from_mode(props.bus_error_mode());
    CAN_STATE.store(state as u8, Ordering::Relaxed);
}

pub fn snapshot_can_diagnostics() -> CanDiagnostics {
    CanDiagnostics {
        tx_err: CAN_TX_ERR.load(Ordering::Relaxed),
        rx_err: CAN_RX_ERR.load(Ordering::Relaxed),
        tx_failures: CAN_TX_FAILS.load(Ordering::Relaxed),
        rx_failures: CAN_RX_FAILS.load(Ordering::Relaxed),
        bus_state: match CAN_STATE.load(Ordering::Relaxed) {
            1 => BusState::ErrorPassive,
            2 => BusState::BusOff,
            _ => BusState::ErrorActive,
        },
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
    fn delay_ms<'a>(&'a mut self, millis: u32) -> impl Future<Output = ()> + 'a {
        async move {
            Timer::after(Duration::from_millis(millis as u64)).await;
        }
    }
}
