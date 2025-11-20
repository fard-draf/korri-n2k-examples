use core::future::Future;
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use embassy_stm32::can::{
    self,
    enums::{BusError, BusErrorMode, FrameCreateError},
    Frame as HalFrame,
};
use embassy_time::{Duration, Timer};
use embedded_can::Id;
use korri_n2k::protocol::transport::{
    can_frame::CanFrame,
    can_id::CanId,
    traits::{can_bus::CanBus, korri_timer::KorriTimer},
};

pub struct Stm32CanBus<'d> {
    inner: can::Can<'d>,
}

impl<'d, const TX_BUF: usize, const RX_BUF: usize> Stm32CanBus<'d, TX_BUF, RX_BUF> {
    pub fn new(inner: can::Can<'d>) -> Self {
        Self { inner }
    }
}

pub struct Stm32CanBusRx<'d, const RX_BUF: usize> {
    rx: can::CanRx<'d>,
}

pub struct Stm32CanBusTx<'d, const TX_BUF: usize> {
    tx: can::CanTx<'d>,
}

impl<'d, const TX_BUF: usize, const RX_BUF: usize> Stm32CanBus<'d, TX_BUF, RX_BUF> {
    pub fn split(self) -> (Stm32CanBusRx<'d, RX_BUF>, Stm32CanBusTx<'d, TX_BUF>) {
        let (tx, rx, _) = self.inner.split();
        (Stm32CanBusRx { rx }, Stm32CanBusTx { tx })
    }
}
// pub struct Stm32CanBus<'d> {
//     inner: can::Can<'d>,
// }

// impl<'d, const TX_BUF: usize, const RX_BUF: usize> Stm32CanBus<'d, TX_BUF, RX_BUF> {
//     pub fn new(inner: can::Can<'d>) -> Self {
//         Self { inner }
//     }
// }

// pub struct Stm32CanBusRx<'d, const RX_BUF: usize> {
//     rx: can::CanRx<'d>,
// }

// pub struct Stm32CanBusTx<'d, const TX_BUF: usize> {
//     tx: can::CanTx<'d>,
// }

// impl<'d, const TX_BUF: usize, const RX_BUF: usize> Stm32CanBus<'d, TX_BUF, RX_BUF> {
//     pub fn split(self) -> (Stm32CanBusRx<'d, RX_BUF>, Stm32CanBusTx<'d, TX_BUF>) {
//         let (tx, rx, _) = self.inner.split();
//         (Stm32CanBusRx { rx }, Stm32CanBusTx { tx })
//     }
// }
