use embassy_time::{Duration, Ticker};

use korri_n2k::protocol::messages::Pgn130310;

pub async fn task_environmental_130310<const N: usize>(
    handle: &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, N>,
) {
    let interval = Pgn130310::PGN_130310_DESCRIPTOR
        .trans_interval
        .unwrap_or(500) as u64;

    let mut ticker = Ticker::every(Duration::from_millis(interval));
    let mut water_temp: f32 = 18.0;

    loop {
        ticker.next().await;

        let mut env_pgn = Pgn130310::new();
        env_pgn.sid = 1;
        env_pgn.water_temperature = water_temp;
        env_pgn.outside_ambient_air_temperature = 22.0;
        env_pgn.atmospheric_pressure = 101325.0;

        water_temp = 18.0 + (water_temp - 18.0 + 0.1) % 5.0;

        let _ = handle.send_pgn(&env_pgn, 130310, 2, None).await;
    }
}
