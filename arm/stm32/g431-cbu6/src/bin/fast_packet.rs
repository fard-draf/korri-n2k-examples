#![no_std]
#![no_main]

use defmt_rtt as _;
use g431_cbu6::{
    app::{idle_forever, run},
    manager_service, tasks,
};
use panic_probe as _;
use shared_core::instances::IDENTITY_3;

#[embassy_executor::main]
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
