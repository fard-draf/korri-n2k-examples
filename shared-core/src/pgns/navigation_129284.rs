use embassy_time::{Duration, Ticker};
use defmt::{info, Debug2Format};
use korri_n2k::protocol::{
    lookups::{BearingMode, DirectionReference, YesNo}, messages::Pgn129284
};

pub async fn task_navigation_129284<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)

{
    let mut ticker = Ticker::every(Duration::from_millis(100));
    let mut distance: f32 = 1000.0;

    loop {
        ticker.next().await;

        let mut nav_pgn = Pgn129284::new();
        nav_pgn.sid = 1;
        nav_pgn.distance_to_waypoint = distance;
        nav_pgn.course_bearing_reference = DirectionReference::Magnetic1;
        nav_pgn.perpendicular_crossed = YesNo::No;
        nav_pgn.arrival_circle_entered = YesNo::No;
        nav_pgn.calculation_type = BearingMode::GreatCircle;
        nav_pgn.eta_time = 0.0;
        nav_pgn.eta_date = 0;
        nav_pgn.bearing_origin_to_destination_waypoint = 45.0;
        nav_pgn.bearing_position_to_destination_waypoint = 47.0;
        nav_pgn.origin_waypoint_number = 1;
        nav_pgn.destination_waypoint_number = 2;
        nav_pgn.destination_latitude = 47.65;
        nav_pgn.destination_longitude = -2.72;

        distance = (distance - 10.0).max(0.0);
        if distance <= 0.0 {
            distance = 1000.0;
        }

        match handle.send_pgn(&nav_pgn, 129284, 2, None).await {
            Ok(_) => {
                info!("PGN 129284 sent successfully");
            }
            Err(e) => {
                info!("Error sending PGN 129284: {:?}", Debug2Format(&e));
            }
        }
    }
}
