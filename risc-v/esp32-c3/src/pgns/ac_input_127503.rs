use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        managment::address_manager::AddressManager,
        messages::Pgn127503,
        transport::fast_packet::builder::FastPacketBuilder,
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_ac_input_127503(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut payload_buffer = [0u8; 256];

    loop {
        ticker.next().await;

        let mut ac_input_pgn = Pgn127503::new();
        ac_input_pgn.instance = 0;
        ac_input_pgn.number_of_lines = 1;

        let payload_len = match ac_input_pgn.to_payload(&mut payload_buffer) {
            Ok(len) => len,
            Err(_) => continue,
        };

        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let fp_builder = FastPacketBuilder::new(127503, my_address, None, &payload_buffer[..payload_len]);

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
