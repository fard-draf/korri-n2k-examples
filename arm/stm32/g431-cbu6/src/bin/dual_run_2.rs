#![no_std]
#![no_main]

//! Second half of the address conflict pair. Flash `dual_run_1` on one board
//! and this one on another: both prefer the same address with a different NAME.

use defmt_rtt as _;
use g431_cbu6::{
    app::{idle_forever, run},
    manager_service, tasks,
};
use panic_probe as _;
use shared_core::instances::IDENTITY_2;

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let (runner, handle) = run(&IDENTITY_2);

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

    idle_forever().await;
}
