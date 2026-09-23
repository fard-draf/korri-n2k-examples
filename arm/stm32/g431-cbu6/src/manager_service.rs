use crate::conf::{CAN_RX_BUF_DEPTH, CAN_TX_BUF_DEPTH};
use crate::ports::{Stm32CanBus, Stm32Timer};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Ticker};
use korri_n2k::protocol::management::{
    address_claiming::engine::ClaimStatus,
    address_supervisor::{AddressHandle, AddressService, ClaimedAddress, SupervisorCommand},
};
use static_cell::StaticCell;

const COMMAND_CAPACITY: usize = 16;

pub type AddressManagerType<'a> = korri_n2k::protocol::management::address_manager::AddressManager<
    'a,
    Stm32CanBus<'static, CAN_TX_BUF_DEPTH, CAN_RX_BUF_DEPTH>,
    Stm32Timer,
>;

pub type ManagerRunner = korri_n2k::protocol::management::address_supervisor::AddressRunner<
    'static,
    Stm32CanBus<'static, CAN_TX_BUF_DEPTH, CAN_RX_BUF_DEPTH>,
    Stm32Timer,
    COMMAND_CAPACITY,
    0,
>;

pub type Handle = &'static AddressHandle<'static, COMMAND_CAPACITY>;

/// Both are `const`, so no allocation and no `StaticCell` is needed.
static COMMAND_CHANNEL: Channel<CriticalSectionRawMutex, SupervisorCommand, COMMAND_CAPACITY> =
    Channel::new();
static CLAIMED: ClaimedAddress = ClaimedAddress::new();

/// The handle is built at runtime, so this one still needs a cell.
static MANAGER_HANDLE: StaticCell<AddressHandle<'static, COMMAND_CAPACITY>> = StaticCell::new();

/// No frame channel: these binaries only talk. Pass one to `AddressService` to
/// read incoming traffic.
pub fn init_manager(manager: AddressManagerType<'static>) -> (ManagerRunner, Handle) {
    let service = AddressService::<_, _, COMMAND_CAPACITY, 0>::new(
        manager,
        Some(&COMMAND_CHANNEL),
        None,
        &CLAIMED,
    );
    let parts = service.into_parts();
    let handle = parts
        .handle
        .expect("command channel ensures handle availability");
    (parts.runner, MANAGER_HANDLE.init(handle))
}

/// The claim campaign starts here, not in `AddressManager::new`.
///
/// `drive` returns only on a bus error, and that error is terminal: the node
/// keeps no address and nothing restarts the loop.
#[embassy_executor::task]
pub async fn address_manager_task(runner: ManagerRunner) {
    defmt::info!("Address supervisor runner spawned");
    if let Err(err) = runner.drive().await {
        defmt::error!("address management stopped: {}", defmt::Debug2Format(&err));
    }
}

/// Report every address-claim state transition without flooding the demo log.
#[embassy_executor::task]
pub async fn address_status_task(handle: Handle) {
    let mut previous_status: Option<Option<ClaimStatus>> = None;
    let mut ticker = Ticker::every(Duration::from_millis(10));

    loop {
        ticker.next().await;
        let status = handle.claim_status();
        if previous_status == Some(status) {
            continue;
        }
        previous_status = Some(status);

        match status {
            None => defmt::warn!(
                "ADDRESS STATE | current address: NONE | claimed address: NONE | status: MANAGER NOT RUNNING"
            ),
            Some(ClaimStatus::Claiming(address)) => defmt::info!(
                "ADDRESS STATE | current address: {=u8:03} | claimed address: NONE | status: CLAIMING (arbitration in progress)",
                address
            ),
            Some(ClaimStatus::Claimed(address)) => defmt::info!(
                "ADDRESS STATE | current address: {=u8:03} | claimed address: {=u8:03} | status: CLAIMED (normal traffic enabled)",
                address,
                address
            ),
            Some(ClaimStatus::CannotClaim) => defmt::warn!(
                "ADDRESS STATE | current address: NONE | claimed address: NONE | status: CANNOT CLAIM (normal traffic disabled)"
            ),
        }
    }
}
