#![no_std]
#![no_main]

use defmt_rtt as _;
use esp_backtrace as _;

esp_bootloader_esp_idf::esp_app_desc!();
use esp32_c3::{
    app::{idle_forever, run},
    manager_service, tasks,
};
use shared_core::instances::IDENTITY_3;

#[esp_hal_embassy::main]
async fn main(spawner: embassy_executor::Spawner) {
    let (runner, handle) = run(&IDENTITY_3);

    spawner
        .spawn(manager_service::address_manager_task(runner))
        .expect("spawn address manager");

    spawner
        .spawn(tasks::engine_127489::task_engine_127489(handle))
        .expect("spawn engine_489 task");

    idle_forever().await;
}
