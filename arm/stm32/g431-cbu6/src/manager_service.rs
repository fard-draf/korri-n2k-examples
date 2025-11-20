
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
static MANAGER_HANDLE: static_cell::StaticCell<ManagerHandle> = static_cell::StaticCell::new();

#[derive(Clone, Copy, Debug)]
pub enum ManagerClientError {
    Serialization,
}

pub struct ManagerHandle {
    handle: korri_n2k::protocol::managment::address_supervisor::AddressHandle<'static, COMMAND_CAPACITY>,
}

impl ManagerHandle {
    pub async fn send_pgn<P: korri_n2k::infra::codec::traits::PgnData>(
        &self,
        data: &P,
        pgn: u32,
        priority: u8,
        destination: Option<u8>,
    ) -> Result<(), ManagerClientError> {
        self.handle
            .send_pgn(data, pgn, priority, destination)
            .await
            .map_err(|err| match err {
                korri_n2k::protocol::managment::address_supervisor::AddressHandleError::Serialization => ManagerClientError::Serialization,
            })
    }

    pub async fn send_frame(&self, frame: & korri_n2k::protocol::transport::can_frame::CanFrame) {
        self.handle.send_frame(frame).await;
    }
}

pub fn init_manager(manager: AddressManagerType) -> (ManagerRunner, &'static ManagerHandle) {
    let chan = COMMAND_CHANNEL.init_with(embassy_sync::channel::Channel::new);

    let service = korri_n2k::protocol::managment::address_supervisor::AddressService::<_, _, COMMAND_CAPACITY, 0>::new(manager, Some(chan), None);

    let parts = service.into_parts();
    let handle = parts
        .handle
        .expect("command channel ensures handle availability");
    let manager_handle = MANAGER_HANDLE.init(ManagerHandle { handle });

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
