use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker};

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::{
        lookups::{AisMessageId, PositionAccuracy, RaimFlag, RepeatIndicator, TimeStamp},
        managment::address_manager::AddressManager,
        messages::Pgn129039,
        transport::{
            fast_packet::builder::FastPacketBuilder,
            traits::pgn_sender::PgnSender,
        },
    },
};

type AddressManagerType = AddressManager<crate::ports::EspCanBus<'static>, crate::timer::EspTimer>;

#[embassy_executor::task]
pub async fn task_ais_class_b_129039(
    manager: &'static Mutex<CriticalSectionRawMutex, AddressManagerType>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(30));
    let mut payload_buffer = [0u8; 256];

    loop {
        ticker.next().await;

        let mut ais_pgn = Pgn129039::new();
        ais_pgn.message_id = AisMessageId::ScheduledClassAPositionReport;
        ais_pgn.repeat_indicator = RepeatIndicator::Initial;
        ais_pgn.user_id = 987654321;
        ais_pgn.longitude = -2.71842;
        ais_pgn.latitude = 47.64425;
        ais_pgn.position_accuracy = PositionAccuracy::High;
        ais_pgn.raim = RaimFlag::NotInUse;
        ais_pgn.time_stamp = TimeStamp::NotAvailable;
        ais_pgn.cog = 90.0;
        ais_pgn.sog = 3.0;

        let payload_len = match ais_pgn.to_payload(&mut payload_buffer) {
            Ok(len) => len,
            Err(_) => continue,
        };

        let my_address = {
            let mgr = manager.lock().await;
            mgr.current_address()
        };

        let fp_builder = FastPacketBuilder::new(129039, my_address, None, &payload_buffer[..payload_len]);

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
