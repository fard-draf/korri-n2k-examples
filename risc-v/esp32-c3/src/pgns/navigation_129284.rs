use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        lookups::{BearingMode, DirectionReference, YesNo},
        managment::address_manager::AddressManager,
        messages::Pgn129284,
        transport::fast_packet::builder::FastPacketBuilder,
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_navigation_129284(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut payload_buffer = [0u8; 256];
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

        let payload_len = match nav_pgn.to_payload(&mut payload_buffer) {
            Ok(len) => len,
            Err(_) => continue,
        };

        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let fp_builder = FastPacketBuilder::new(129284, my_address, None, &payload_buffer[..payload_len]);

        {
            let mut mgr = manager.lock().await;
            for frame_result in fp_builder.build() {
                match frame_result {
                    Ok(frame) => {
                        if let Err(_e) = mgr.send(&frame).await {
                            // Gestion d'erreur simplifiée
                        }
                    }
                    Err(_e) => {
                        break;
                    }
                }
            }
        }
    }
}
