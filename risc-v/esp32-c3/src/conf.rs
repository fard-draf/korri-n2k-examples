use esp_hal::twai::BaudRate;

/// TWAI RX on GPIO10, TX on GPIO9.
pub const N2K_BITRATE: BaudRate = BaudRate::B250K;
