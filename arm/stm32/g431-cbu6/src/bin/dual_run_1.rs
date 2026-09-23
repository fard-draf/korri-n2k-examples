#![no_std]
#![no_main]

//! Stronger half of the address-conflict pair. Its lower ISO NAME wins.

use defmt_rtt as _;
use g431_cbu6::{
    app::{idle_forever, run},
    manager_service, tasks,
};
use panic_probe as _;
use shared_core::instances::IDENTITY_1;

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let (runner, handle) = run(&IDENTITY_1);

    defmt::info!("DUAL_RUN_1 | fixed address strategy | lower ISO NAME wins the shared address");

    spawner
        .spawn(manager_service::address_manager_task(runner))
        .expect("spawn address manager");

    spawner
        .spawn(manager_service::address_status_task(handle))
        .expect("spawn address status");

    spawner
        .spawn(tasks::ac_input_127503::task_ac_input_127503(handle))
        .expect("spawn ac_input task");

    spawner
        .spawn(tasks::position_129025::task_position_129025(handle))
        .expect("spawn position task");

    spawner
        .spawn(tasks::engine_127488::task_engine_127488(handle))
        .expect("spawn engine_488 task");

    idle_forever().await;
}
