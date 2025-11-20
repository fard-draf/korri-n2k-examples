use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        lookups::{DirectionReference, DirectionRudder, SteeringMode, TurnMode, YesNo},
        managment::address_manager::AddressManager,
        messages::Pgn127237,
        transport::{
            can_frame::CanFrame,
            can_id::CanId,
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_heading_control_127237(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let mut ticker = Ticker::every(Duration::from_millis(250));
    let mut buffer = [0u8; 8];
    let mut rudder_angle: f32 = 0.0;

    loop {
        ticker.next().await;

        let mut heading_pgn = Pgn127237::new();
        heading_pgn.rudder_limit_exceeded = YesNo::No;
        heading_pgn.off_heading_limit_exceeded = YesNo::No;
        heading_pgn.off_track_limit_exceeded = YesNo::No;
        heading_pgn.override_field = YesNo::No;
        heading_pgn.steering_mode = SteeringMode::MainSteering;
        heading_pgn.turn_mode = TurnMode::RudderLimitControlled;
        heading_pgn.heading_reference = DirectionReference::Magnetic1;
        heading_pgn.commanded_rudder_direction = DirectionRudder::NoOrder;
        heading_pgn.commanded_rudder_angle = rudder_angle;
        heading_pgn.heading_to_steer_course = 0.0;
        heading_pgn.track = 0.0;

        rudder_angle = (rudder_angle + 0.5) % 360.0;

        let payload_len = match heading_pgn.to_payload(&mut buffer) {
            Ok(len) => len,
            Err(_) => continue,
        };

        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let can_id = CanId::builder(127237, my_address)
            .with_priority(2)
            .build()
            .expect("La construction doit reussir");

        let frame = CanFrame {
            id: can_id,
            data: buffer,
            len: payload_len,
        };

        {
            let mut mgr = manager.lock().await;
            let _ = mgr.send(&frame).await;
        }
    }
}
