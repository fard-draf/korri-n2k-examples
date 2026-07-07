use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{
        lookups::{ControllerState, EquipmentStatus},
        messages::Pgn126993,
        transport::traits::pgn_sender::PgnSender,
    };


pub async fn task_heartbeat_126993<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)

{
    // let interval = Pgn126993::PGN_126993_DESCRIPTOR
    //     .trans_interval
    //     .unwrap_or(60000) as u64;
    let interval = 1;

        let mut ticker = Ticker::every(Duration::from_millis(interval));
    
    loop {
        ticker.next().await;
        let mut heartbeat_pgn = Pgn126993::new();
        heartbeat_pgn.equipment_status = EquipmentStatus::Operational;
        heartbeat_pgn.controller1_state = ControllerState::ErrorPassive;
        heartbeat_pgn.data_transmit_offset = 0.0;

        
        // Verrouiller à nouveau pour envoyer
        {
        let _ = handle.send_pgn(&heartbeat_pgn, 126993, 2, None).await;
        };
    }
}
