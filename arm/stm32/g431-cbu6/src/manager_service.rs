use crate::conf::{CAN_RX_BUF_DEPTH, CAN_TX_BUF_DEPTH};
use crate::ports::{Stm32CanBus, Stm32Timer};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use korri_n2k::protocol::managment::address_supervisor::{
    AddressHandle, AddressService, AddressSupervisorRunError, SupervisorCommand,
};
use static_cell::StaticCell;

const COMMAND_CAPACITY: usize = 16;

pub type AddressManagerType<'a> = korri_n2k::protocol::managment::address_manager::AddressManager<
    'a,
    Stm32CanBus<'static, CAN_TX_BUF_DEPTH, CAN_RX_BUF_DEPTH>,
    Stm32Timer,
>;

pub type ManagerRunner = korri_n2k::protocol::managment::address_supervisor::AddressRunner<
    'static,
    Stm32CanBus<'static, CAN_TX_BUF_DEPTH, CAN_RX_BUF_DEPTH>,
    Stm32Timer,
    COMMAND_CAPACITY,
    0,
>;

pub type Handle = &'static AddressHandle<'static, COMMAND_CAPACITY>;

static COMMAND_CHANNEL: StaticCell<
    Channel<CriticalSectionRawMutex, SupervisorCommand, COMMAND_CAPACITY>,
> = StaticCell::new();
static MANAGER_HANDLE: StaticCell<AddressHandle<'static, COMMAND_CAPACITY>> = StaticCell::new();

pub fn init_manager(manager: AddressManagerType<'static>) -> (ManagerRunner, Handle) {
    let chan = COMMAND_CHANNEL.init_with(Channel::new);
    let service = AddressService::<_, _, COMMAND_CAPACITY, 0>::new(manager, Some(chan), None);
    let parts = service.into_parts();
    let handle = parts
        .handle
        .expect("command channel ensures handle availability");
    (parts.runner, MANAGER_HANDLE.init(handle))
}

#[embassy_executor::task]
pub async fn address_manager_task(runner: ManagerRunner) {
    defmt::info!("Address supervisor runner spawned");
    if let Err(err) = runner.drive().await {
        match err {
            AddressSupervisorRunError::Receive(_) => defmt::error!("runner stopped: recv error"),
            AddressSupervisorRunError::Send(_) => defmt::error!("runner stopped: send error"),
            AddressSupervisorRunError::SendPgn(_) => {
                defmt::error!("runner stopped: send_pgn error")
            }
        }
        loop {}
    }
}
