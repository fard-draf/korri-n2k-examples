use defmt::{info, Debug2Format};
use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{messages::Pgn129025};

const MAX_LAT: f32 = 47.40;
const MIN_LAT: f32 = 44.40;
const MAX_LONG: f32 = -3.00;
const MIN_LONG: f32 = -5.00;

pub async fn task_position_129025<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)


{
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut start_lat = 46.00;
    let mut start_long = -3.70;
    loop {
        ticker.next().await;
        let mut position_pgn = Pgn129025::new();
        position_pgn.latitude = start_lat ;
        position_pgn.longitude = start_long ;

        if position_pgn.latitude > MAX_LAT {
            start_lat = MIN_LAT;
            position_pgn.latitude = start_lat;
        } else {
            start_lat += 0.03;
        }

        if position_pgn.longitude > MAX_LONG {
            start_long = MIN_LONG;
            position_pgn.longitude = start_long;
        } else {
            start_long += 0.02;
        }

        match handle.send_pgn(&position_pgn, 129025, 2, None).await {


            Ok(_) => {
                info!("PGN 129284 sent successfully");
            }
            Err(e) => {
                info!("Error sending PGN 129284: {:?}", Debug2Format(&e));
            }
        }
    }
}
