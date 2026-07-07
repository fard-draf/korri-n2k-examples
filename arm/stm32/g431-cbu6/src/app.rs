use defmt::{error, info};

use crate::{
    instances::IsoIdentity,
    manager_service::{self, Handle, ManagerRunner},
    ports::{Stm32CanBus, Stm32Timer},
    starter::{init_bsp, init_can},
};

pub async fn run(identity: &IsoIdentity) -> (ManagerRunner, Handle) {
    let p = init_bsp();
    let can = init_can(p);

    info!("Embassy up. Bringing up korri-n2k stack.");

    let iso_name = korri_n2k::protocol::managment::iso_name::IsoName::builder()
        .unique_number(identity.unique_number)
        .manufacturer_code(identity.manufacturer_code)
        .device_function(identity.device_function)
        .device_class(identity.device_class)
        .device_instance(identity.device_instance)
        .system_instance(identity.system_instance)
        .industry_group(identity.industry_group)
        .arbitrary_address_capable(true)
        .build();
    info!("ISO NAME: 0x{=u64:X}", iso_name.raw());

    let manager = match korri_n2k::protocol::managment::address_manager::AddressManager::new(
        Stm32CanBus::new(can),
        Stm32Timer::new(),
        iso_name.raw(),
        identity.preferred_address,
    )
    .await
    {
        Ok(mgr) => {
            info!("Address claimed: {}", mgr.current_address());
            mgr
        }
        Err(_) => {
            error!("Failed to claim an address on the NMEA2000 bus");
            panic!("address claim failed");
        }
    };

    manager_service::init_manager(manager)
}

pub async fn idle_forever() -> ! {
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(5)).await;
        defmt::trace!("main idle");
    }
}
