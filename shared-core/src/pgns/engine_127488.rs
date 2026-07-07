use embassy_time::{Duration, Ticker};
use defmt::{Debug2Format, error, info};

use korri_n2k::protocol::{
        lookups::EngineInstance,
        messages::Pgn127488,
    };

type AddressHandle<const N: usize> = korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>;

pub async fn task_engine_127488<const N: usize>(
    handle: &'static AddressHandle<N>,
)

{
    let mut ticker = Ticker::every(Duration::from_millis(100));
    
    let mut tilt_trim: i8 = 0;
    let mut rpm: u16 = 0;

    loop {
        ticker.next().await;
        let mut engine_pgn = Pgn127488::new();
        engine_pgn.instance = EngineInstance::SingleEngineOrDualEnginePort;
        engine_pgn.speed = (1000.0 + (rpm as f32)) % 2500.0;
        engine_pgn.boost_pressure = 1478.0;
        engine_pgn.tilt_trim = tilt_trim;

        rpm = rpm.wrapping_add(1);
        tilt_trim = (tilt_trim + 1) % 101;

        {
            match handle.send_pgn(&engine_pgn, 127488, 2, None).await {
                Ok(_) => {
                    info!("PGN 127488 sent successfully");
                }
                Err(e) => {
                    error!("Error sending PGN 127503: {:?}", Debug2Format(&e));
                }
            }
        }
    }
}
