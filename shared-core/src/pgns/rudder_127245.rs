use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{
        lookups::DirectionRudder,
        messages::Pgn127245,
    };


pub async fn task_rudder_127245<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)

{
    let interval = Pgn127245::PGN_127245_DESCRIPTOR
        .trans_interval
        .unwrap_or(100) as u64;

    
    let mut ticker = Ticker::every(Duration::from_millis(interval));
        let mut position: f32 = 0.0;

    loop {
        ticker.next().await;

        let mut rudder_pgn = Pgn127245::new();
        rudder_pgn.instance = 0;
        rudder_pgn.direction_order = DirectionRudder::NoOrder;
        rudder_pgn.angle_order = 0.0;
        rudder_pgn.position = position;

        position = (position + 0.5) % 360.0;

        

        

        

        

        let _ = handle.send_pgn(&rudder_pgn, 127245, 2, None).await;
    }
}
