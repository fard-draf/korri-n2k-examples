use defmt::{Debug2Format, info};
use korri_n2k::protocol::management::{address_manager::AddressManager, iso_name::IsoName};

use shared_core::instances::IsoIdentity;

use crate::{
    manager_service::{self, Handle, ManagerRunner},
    ports::{EspCanBus, EspTimer},
    starter::{init_bsp, init_can},
};

/// Bring up the board and build the N2K stack. Nothing touches the bus yet:
/// the address claim starts when the runner is spawned.
pub fn run(identity: &IsoIdentity) -> (ManagerRunner, Handle) {
    let p = init_bsp();
    let can = init_can(p);

    info!("Embassy up. Bringing up korri-n2k stack.");

    let iso_name = IsoName::builder()
        .unique_number(identity.unique_number)
        .manufacturer_code(identity.manufacturer_code)
        .device_function(identity.device_function)
        .device_class(identity.device_class)
        .device_instance(identity.device_instance)
        .system_instance(identity.system_instance)
        .industry_group(identity.industry_group)
        .arbitrary_address_capable(identity.is_arbitrary_address_capable())
        .build();
    info!(
        "ISO NAME: 0x{=u64:X}, strategy {}",
        iso_name.raw(),
        Debug2Format(&identity.strategy)
    );

    // Synchronous. It only fails when the NAME contradicts the strategy, and the
    // identity derives the AAC bit from that strategy, so it cannot.
    let manager = AddressManager::new(
        EspCanBus::new(can),
        EspTimer::new(),
        iso_name,
        identity.strategy,
    )
    .expect("NAME and address claim strategy disagree");

    manager_service::init_manager(manager)
}

pub async fn idle_forever() -> ! {
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(5)).await;
        defmt::trace!("main idle");
    }
}
