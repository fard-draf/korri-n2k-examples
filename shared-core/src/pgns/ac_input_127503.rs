use embassy_time::{Duration, Ticker};

use defmt::{Debug2Format, error, info};
use korri_n2k::protocol::messages::Pgn127503;

pub async fn task_ac_input_127503<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(1));

    loop {
        ticker.next().await;

        let mut ac_input_pgn = Pgn127503::new();
        ac_input_pgn.instance = 210;
        ac_input_pgn.number_of_lines = 185;

        {
            match handle.send_pgn(&ac_input_pgn, 127503, 2, None).await {
                Ok(_) => {
                    info!("PGN 127503 sent successfully");
                }
                Err(e) => {
                    error!("Error sending PGN 127503: {:?}", Debug2Format(&e));
                }
            }
        }
    }
}
