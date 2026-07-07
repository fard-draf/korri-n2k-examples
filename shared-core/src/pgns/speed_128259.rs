use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{
        lookups::WaterReference,
        messages::Pgn128259,
    };


pub async fn task_speed_128259<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)

{
    defmt::info!("task_speed démarrée");
    let interval = Pgn128259::PGN_128259_DESCRIPTOR
        .trans_interval
        .unwrap_or(5000) as u64;
        let mut ticker = Ticker::every(Duration::from_millis(interval));
    
    loop {
        ticker.next().await;
        let mut speed = Pgn128259::new();
        speed.sid = 240;
        speed.speed_water_referenced = 5.5;
        speed.speed_ground_referenced = 55.8;
        speed.speed_water_referenced_type = WaterReference::PaddleWheel;
        speed.speed_direction = 158;

        

        // Verrouiller le mutex pour accéder au manager
        

        

        

        // Verrouiller à nouveau pour envoyer
        let _ = handle.send_pgn(&speed, 128259, 2, None).await;;
    }
}
