#![no_std]
#![no_main]

use defmt_rtt as _;
use embassy_executor::Spawner;
use esp_backtrace as _;

esp_bootloader_esp_idf::esp_app_desc!();
use esp32_s3::{
    app::{idle_forever, run},
    instances::inst1::IDENTITY,
    manager_service, tasks,
};

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let (runner, handle) = run(&IDENTITY).await;

    spawner
        .spawn(manager_service::address_manager_task(runner))
        .expect("spawn address manager");

    spawner
        .spawn(tasks::ac_input_127503::task_ac_input_127503(handle))
        .expect("spawn ac_input task");

    spawner
        .spawn(tasks::position_129025::task_position_129025(handle))
        .expect("spawn position task");

    spawner
        .spawn(tasks::engine_127488::task_engine_127488(handle))
        .expect("spawn engine_488 task");

    spawner
        .spawn(tasks::engine_127489::task_engine_127489(handle))
        .expect("spawn engine_489 task");

    idle_forever().await;
}
