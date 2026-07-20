use esp_hal::twai::BaudRate;

/// TWAI RX on GPIO41, TX on GPIO42.
pub const N2K_BITRATE: BaudRate = BaudRate::B250K;

/// Same bitrate in bit/s for the capture header: `BaudRate` does not expose its
/// numeric value.
pub const N2K_BITRATE_BPS: u32 = 250_000;

pub const DIAG_PERIOD_MS: u64 = 1_000;
