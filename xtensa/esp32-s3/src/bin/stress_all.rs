#![no_std]
#![no_main]

use defmt_rtt as _;
use esp_backtrace as _;

esp_bootloader_esp_idf::esp_app_desc!();
use esp32_s3::{
    app::{idle_forever, run},
    instances::inst5::IDENTITY,
    manager_service, tasks,
};

#[esp_hal_embassy::main]
async fn main(spawner: embassy_executor::Spawner) {
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
    spawner
        .spawn(tasks::heartbeat_126993::task_heartbeat_126993(handle))
        .expect("spawn heartbeat task");
    spawner
        .spawn(tasks::depth_128267::task_depth_128267(handle))
        .expect("spawn depth task");
    spawner
        .spawn(tasks::speed_128259::task_speed_128259(handle))
        .expect("spawn speed task");
    spawner
        .spawn(tasks::rudder_127245::task_rudder_127245(handle))
        .expect("spawn rudder task");
    spawner
        .spawn(tasks::datum_129044::task_datum_129044(handle))
        .expect("spawn datum task");
    spawner
        .spawn(tasks::navigation_129284::task_navigation_129284(handle))
        .expect("spawn navigation task");
    spawner
        .spawn(tasks::heading_control_127237::task_heading_control_127237(
            handle,
        ))
        .expect("spawn heading_control task");
    spawner
        .spawn(tasks::environmental_130310::task_environmental_130310(
            handle,
        ))
        .expect("spawn environmental task");
    spawner
        .spawn(tasks::alert_text_126985::task_alert_text_126985(handle))
        .expect("spawn alert_text task");
    spawner
        .spawn(tasks::ais_class_a_129038::task_ais_class_a_129038(handle))
        .expect("spawn ais_class_a task");
    spawner
        .spawn(tasks::ais_class_b_129039::task_ais_class_b_129039(handle))
        .expect("spawn ais_class_b task");

    idle_forever().await;
}
