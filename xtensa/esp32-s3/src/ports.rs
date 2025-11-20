use core::future::Future;
use embedded_can::{Frame, Id};
use embassy_time::{with_timeout, Duration};
use esp_hal::{
    twai::{EspTwaiError, EspTwaiFrame, ExtendedId as EspExtendedId, Twai, TwaiRx, TwaiTx},
    Async,
};
use korri_n2k::protocol::transport::{
    can_frame::CanFrame, can_id::CanId, traits::can_bus::CanBus, CAN_SEND_TIMEOUT_MS,
};

/// Adaptateur pour le contrôleur TWAI ESP32 implémentant le trait [`CanBus`] de korri-n2k.
///
/// Cette implémentation fournit :
/// - **Timeout automatique** sur l'envoi (100ms) pour éviter les blocages
/// - **Support asynchrone** via Embassy
/// - **Conformité NMEA2000** avec gestion des trames étendues 29 bits
///
/// # Limitations matérielles ESP32
///
/// Le contrôleur TWAI ESP32 possède un **buffer TX limité à 3 trames**.
/// Les délais inter-frame Fast Packet gérés par korri-n2k sont essentiels
/// pour éviter la saturation de ce buffer.
///
/// # Exemple
///
/// ```rust,ignore
/// let can_config = esp_hal::twai::TwaiConfiguration::new(
///     peripherals.TWAI0,
///     can_rx_pin,
///     can_tx_pin,
///     BaudRate::B250K,
///     TwaiMode::Normal,
/// ).into_async();
///
/// let can_peripheral = can_config.start();
/// let can_bus = EspCanBus::new(can_peripheral);
/// ```
pub struct EspCanBus<'d> {
    can: Twai<'d, Async>,
}

impl<'d> EspCanBus<'d> {
    /// Crée un nouvel adaptateur CAN à partir d'un périphérique TWAI ESP32.
    pub fn new(can: Twai<'d, Async>) -> Self {
        Self { can }
    }

    /// Split le bus CAN en parties TX et RX séparées pour usage concurrent.
    ///
    /// Permet d'avoir des tâches distinctes pour l'émission et la réception.
    pub fn split(self) -> (EspCanBusRx<'d>, EspCanBusTx<'d>) {
        let (rx, tx) = self.can.split();
        (EspCanBusRx { rx }, EspCanBusTx { tx })
    }
}

// Wrapper pour la partie RX uniquement
pub struct EspCanBusRx<'d> {
    rx: TwaiRx<'d, Async>,
}

impl<'d> EspCanBusRx<'d> {
    pub async fn recv(&mut self) -> Result<CanFrame, EspTwaiError> {
        let frame = match self.rx.receive_async().await {
            Ok(frame) => frame,
            Err(e) => {
                esp_println::println!("Erreur TWAI (RX split) : {:?}", e);
                return Err(e);
            }
        };

        let data_frame = frame.data();
        let len = frame.dlc();

        let id = match frame.id() {
            Id::Standard(_) => {
                esp_println::println!("Frame standard (RX split) ignorée");
                return Err(EspTwaiError::BusOff);
            }
            Id::Extended(ext) => ext.as_raw(),
        };

        let mut data = [0u8; 8];
        data[..len].copy_from_slice(data_frame);

        let can_frame = CanFrame {
            id: CanId(id),
            data,
            len,
        };

        if can_frame.id.pgn() == 60928 {
            esp_println::println!("<<< RX CLAIM from SA={}", can_frame.id.source_address());
        }

        Ok(can_frame)
    }
}

// Wrapper pour la partie TX uniquement
pub struct EspCanBusTx<'d> {
    tx: TwaiTx<'d, Async>,
}

impl<'d> EspCanBusTx<'d> {
    pub async fn send(&mut self, frame: &CanFrame) -> Result<(), EspTwaiError> {
        let ext_id = EspExtendedId::new(frame.id.0).unwrap();
        let twai_frame = EspTwaiFrame::new(ext_id, &frame.data[..frame.len]).unwrap();

        if frame.id.pgn() == 60928 {
            esp_println::println!(">>> TX CLAIM from SA={}", frame.id.source_address());
        }

        self.tx.transmit_async(&twai_frame).await
    }
}

impl<'d> CanBus for EspCanBus<'d> {
    type Error = EspTwaiError;

    fn send<'a>(
        &'a mut self,
        frame: &'a CanFrame,
    ) -> impl Future<Output = Result<(), Self::Error>> + 'a {
        async move {
            let ext_id = EspExtendedId::new(frame.id.0).unwrap();
            let twai_frame = EspTwaiFrame::new(ext_id, &frame.data[..frame.len]).unwrap();

            if frame.id.pgn() == 60928 {
                esp_println::println!(">>> TX CLAIM from SA={}", frame.id.source_address());
            } else {
                esp_println::println!(
                    ">>> TX PGN={} from SA={}",
                    frame.id.pgn(),
                    frame.id.source_address()
                );
            }

            // Envoi avec timeout pour éviter blocage infini si buffer TX plein
            // Timeout recommandé NMEA2000 : 100ms pour 1 trame @ 250kbps
            with_timeout(
                Duration::from_millis(CAN_SEND_TIMEOUT_MS as u64),
                self.can.transmit_async(&twai_frame),
            )
            .await
            .map_err(|_| EspTwaiError::BusOff)? // Timeout converti en erreur BusOff
        }
    }

    fn recv<'a>(&'a mut self) -> impl Future<Output = Result<CanFrame, Self::Error>> + 'a {
        async move {
            let frame = match self.can.receive_async().await {
                Ok(frame) => frame,
                Err(e) => {
                    esp_println::println!("Erreur TWAI en réception : {:?}", e);
                    return Err(e);
                }
            };

            let data_frame = frame.data();
            let len = frame.dlc();

            let id = match frame.id() {
                Id::Standard(_) => {
                    esp_println::println!("Frame standard reçue → rejet");
                    return Err(EspTwaiError::BusOff);
                }
                Id::Extended(ext) => ext.as_raw(),
            };

            let mut data = [0u8; 8];
            data[..len].copy_from_slice(data_frame);

            let can_frame = CanFrame {
                id: CanId(id),
                data,
                len,
            };

            if can_frame.id.pgn() == 60928 {
                esp_println::println!("<<< RX CLAIM from SA={}", can_frame.id.source_address());
            }

            Ok(can_frame)
        }
    }
}
