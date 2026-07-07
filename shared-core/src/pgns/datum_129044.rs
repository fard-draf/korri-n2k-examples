use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{
        messages::Pgn129044,
        transport::traits::pgn_sender::PgnSender,
    };


pub async fn task_datum_129044<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)

{
    let interval = Pgn129044::PGN_129044_DESCRIPTOR
        .trans_interval
        .unwrap_or(10000) as u64;

    
    let mut ticker = Ticker::every(Duration::from_millis(interval));
    
    loop {
        ticker.next().await;

        let mut datum_pgn = Pgn129044::new();
        // WGS84 datum
        datum_pgn.local_datum = [b'W', b'G', b'S', b'8'];
        datum_pgn.delta_latitude = 0.0;
        datum_pgn.delta_longitude = 0.0;
        datum_pgn.delta_altitude = 0.0;
        datum_pgn.reference_datum = [b'W', b'G', b'S', b'8'];

        

        

        

        let _ = handle.send_pgn(&datum_pgn, 129044, 2, None).await;
    }
}
