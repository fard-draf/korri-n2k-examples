use core::{any::Any, hash::BuildHasherDefault};

use esp_println::println;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use crate::{ports::EspCanBus, timer::EspTimer};
use korri_n2k::{
    infra::codec::{
        engine,
        traits::{PgnData, ToPayload},
    },
    protocol::{
        lookups::{
            ControllerState, EngineInstance, EngineStatus1, EngineStatus2, EquipmentStatus,
            PgnErrorCode, WaterReference,
        },
        managment::address_manager::AddressManager,
        messages::Pgn127489,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
            fast_packet::{assembler::FastPacketAssembler, builder::FastPacketBuilder},
            traits::{can_bus, korri_timer::KorriTimer, pgn_sender::PgnSender},
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_engine_127489(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let mut ticker = Ticker::every(Duration::from_millis(200));
    let mut tilt: u8 = 0;

    loop {
        ticker.next().await;
        let mut engine_pgn = Pgn127489::new();
        engine_pgn.instance = EngineInstance::SingleEngineOrDualEnginePort;
        engine_pgn.oil_pressure = (50.0 + (tilt as f32)) % 151.0;
        engine_pgn.oil_temperature = (10.0 + (tilt as f32)) % 90.0;
        engine_pgn.temperature = (0.0 + (tilt as f32)) % 90.0;
        engine_pgn.alternator_potential = (12.4 + (tilt as f32)) % 14.2;
        engine_pgn.fuel_rate = (100.0 - (tilt as f32)) % 100.0;
        engine_pgn.total_engine_hours = 15201 + (tilt as u32);
        engine_pgn.coolant_pressure = (123.0 + (tilt as f32)) % 150.0;
        engine_pgn.fuel_pressure = (168.7 - tilt as f32) % 150.0;
        engine_pgn.set_discrete_status1_bit(EngineStatus1::LowOilLevel, true);
        engine_pgn.set_discrete_status2_bit(EngineStatus2::EngineCommError, true);
        engine_pgn.engine_load = 1 + (tilt as i8) % 100;
        engine_pgn.engine_torque = 1 + (tilt as i8) % 100;

        tilt = tilt.wrapping_add(1);

        let mut buffer = [0u8; 200];

        // Sérialiser le PGN - si ça échoue, on skip cette itération
        let len = match engine_pgn.to_payload(&mut buffer) {
            Ok(len) => {
                len
            },
            Err(e) => {
                // Erreur de sérialisation, on saute cette itération
                continue;
            }
        };

        let builder = FastPacketBuilder::new(127489, 142, None, &buffer[..len]);

        // Lock une seule fois pour envoyer toutes les frames
        let mut mgr = manager.lock().await;
        for frame_result in builder.build() {
            match frame_result {
                Ok(frame) => {
                    if let Err(e) = mgr.send(&frame).await {
                        // Erreur d'envoi - on continue avec la prochaine frame
                    }
                }
                Err(_e) => {
                    // Erreur de construction de frame - on arrête l'envoi
                    break;
                }
            }
        }
        drop(mgr); // Libérer le lock explicitement
    }
}
