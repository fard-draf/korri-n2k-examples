use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::{
        lookups::{AisMessageId, PositionAccuracy, RaimFlag, RepeatIndicator, TimeStamp},
        messages::Pgn129038,
    };

pub async fn task_ais_class_a_129038<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
)


 {
    let mut ticker = Ticker::every(Duration::from_secs(6));
    
    loop {
        ticker.next().await;
        let mut ais_pgn = Pgn129038::new();
        ais_pgn.message_id = AisMessageId::ScheduledClassAPositionReport;
        ais_pgn.repeat_indicator = RepeatIndicator::Initial;
        ais_pgn.user_id = 123456789;
        ais_pgn.longitude = -2.71842;
        ais_pgn.latitude = 47.64425;
        ais_pgn.position_accuracy = PositionAccuracy::High;
        ais_pgn.raim = RaimFlag::NotInUse;
        ais_pgn.time_stamp = TimeStamp::NotAvailable;
        ais_pgn.cog = 45.0;
        ais_pgn.sog = 5.0;

        

        

        

        let _ = handle.send_pgn(&ais_pgn, 129038, 2, None).await;
    }
}
