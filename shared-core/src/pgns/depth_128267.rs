use embassy_time::{Duration, Ticker};
use defmt::{Debug2Format, error, info};

use korri_n2k::protocol::{
        messages::Pgn128267,
        transport::traits::pgn_sender::PgnSender,
    };


pub async fn task_depth_128267<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)

{
    let mut ticker = Ticker::every(Duration::from_secs(1));
    
    let mut sim_depth: u16 = 1;

    loop {
        ticker.next().await;
        let mut depth_pgn = Pgn128267::new();
        depth_pgn.sid = 42;
        depth_pgn.depth = (25.0 + ((sim_depth) as f32)) % 255.0;
        depth_pgn.offset = 0.0;
        depth_pgn.range = 50.0;

        sim_depth = sim_depth.wrapping_add(1);

        {
            match handle.send_pgn(&depth_pgn, 128267, 2, None).await {
                Ok(_) => {
                    info!("PGN 128267 sent successfully");
                }
                Err(e) => {
                    error!("Error sending PGN 128267: {:?}", Debug2Format(&e));
                }
            }
        }
    }

        

        
}
