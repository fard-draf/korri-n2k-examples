use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::Channel,
};
use static_cell::StaticCell;

use korri_n2k::{
    infra::codec::traits::PgnData,
    protocol::managment::{
        address_manager::AddressManager,
        address_supervisor::{
            AddressHandle, AddressHandleError, AddressRunner, AddressService, SupervisorCommand,
        },
    },
    protocol::transport::can_frame::CanFrame,
};
use crate::{ports::EspCanBus, timer::EspTimer};

type AddressManagerType = AddressManager<EspCanBus<'static>, EspTimer>;
type ManagerRunner =
    AddressRunner<'static, EspCanBus<'static>, EspTimer, COMMAND_CAPACITY, 0>;

const COMMAND_CAPACITY: usize = 16;

static COMMAND_CHANNEL: StaticCell<
    Channel<CriticalSectionRawMutex, SupervisorCommand, COMMAND_CAPACITY>,
> = StaticCell::new();
static MANAGER_HANDLE: StaticCell<AddressHandle<'static, COMMAND_CAPACITY>> = StaticCell::new();

pub fn init_manager(
    manager: AddressManagerType,
) -> (ManagerRunner, &'static AddressHandle<'static, COMMAND_CAPACITY>) {
    let channel = COMMAND_CHANNEL.init_with(Channel::new);

    let service = AddressService::<_, _, COMMAND_CAPACITY, 0>::new(
        manager,
        Some(channel),
        None,
    );

    let parts = service.into_parts();
    let handle = parts
        .handle
        .expect("command channel ensures handle availability");
    let manager_handle = MANAGER_HANDLE.init(handle);

    (parts.runner, manager_handle)
}

#[embassy_executor::task]
pub async fn address_manager_task(runner: ManagerRunner) {
    if let Err(_err) = runner.drive().await {
        defmt::warn!(
            "Address supervisor stopped:",
        );
    }
}
