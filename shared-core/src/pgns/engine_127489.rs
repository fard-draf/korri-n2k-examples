use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{
    lookups::{EngineInstance, EngineStatus1, EngineStatus2},
    messages::Pgn127489,
};

pub async fn task_engine_127489<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
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

        // engine_pgn.instance = EngineInstance::SingleEngineOrDualEnginePort;
        // engine_pgn.oil_pressure = 50.0;
        // engine_pgn.oil_temperature = 10.0;
        // engine_pgn.temperature = 70.0;
        // engine_pgn.alternator_potential = 12.4;
        // engine_pgn.fuel_rate = 95.0;
        // engine_pgn.total_engine_hours = 15200;
        // engine_pgn.coolant_pressure = 152.0;
        // engine_pgn.fuel_pressure = 168.7;
        // engine_pgn.set_discrete_status1_bit(EngineStatus1::LowOilLevel, true);
        // engine_pgn.set_discrete_status2_bit(EngineStatus2::EngineCommError, true);
        // engine_pgn.engine_load = 55;
        // engine_pgn.engine_torque = 70;

        tilt = tilt.wrapping_add(1);

        let _ = handle.send_pgn(&engine_pgn, 127489, 2, None).await;
    }
}
