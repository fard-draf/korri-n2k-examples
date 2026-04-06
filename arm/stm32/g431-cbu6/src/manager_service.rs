use crate::{CAN_RX_BUF_DEPTH, CAN_TX_BUF_DEPTH, Stm32Timer, ports::Stm32CanBus};

type AddressManagerType =
    korri_n2k::protocol::managment::address_manager::AddressManager<Stm32CanBus<'static, CAN_TX_BUF_DEPTH, CAN_RX_BUF_DEPTH>, Stm32Timer>;

type ManagerRunner = korri_n2k::protocol::managment::address_supervisor::AddressRunner<
    'static,
    Stm32CanBus<'static, CAN_TX_BUF_DEPTH, CAN_RX_BUF_DEPTH>,
    Stm32Timer,
    COMMAND_CAPACITY,
    0,
>;

const COMMAND_CAPACITY: usize = 16;

static COMMAND_CHANNEL: static_cell::StaticCell<
    embassy_sync::channel::Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        korri_n2k::protocol::managment::address_supervisor::SupervisorCommand,
    COMMAND_CAPACITY>,
> = static_cell::StaticCell::new();
static MANAGER_HANDLE: static_cell::StaticCell<korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, COMMAND_CAPACITY>> = static_cell::StaticCell::new();

pub fn init_manager(manager: AddressManagerType) -> (ManagerRunner, &'static korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, COMMAND_CAPACITY>) {
    let chan = COMMAND_CHANNEL.init_with(embassy_sync::channel::Channel::new);

    let service = korri_n2k::protocol::managment::address_supervisor::AddressService::<_, _, COMMAND_CAPACITY, 0>::new(manager, Some(chan), None);

    let parts = service.into_parts();
    let handle = parts
        .handle
        .expect("command channel ensures handle availability");
    let manager_handle = MANAGER_HANDLE.init(handle);

    (parts.runner, manager_handle)
}

#[embassy_executor::task]
pub async fn address_manager_task(runner: ManagerRunner) {
    defmt::info!("Address supervisor runner spawned");
    if let Err(err) = runner.drive().await {
        let _ = err;
        defmt::warn!("Address supervisor stopped");
    }
}
