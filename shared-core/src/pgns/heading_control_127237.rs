use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{
        lookups::{DirectionReference, DirectionRudder, SteeringMode, TurnMode, YesNo},
        messages::Pgn127237,
        transport::traits::pgn_sender::PgnSender,
    };


pub async fn task_heading_control_127237<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)

{
    let mut ticker = Ticker::every(Duration::from_millis(250));
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

        

        

        

        

        let _ = handle.send_pgn(&heading_pgn, 127237, 2, None).await;
    }
}
