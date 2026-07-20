#![no_std]
#![no_main]

//! Passive, timestamped NMEA2000 sniffer.
//!
//! Every frame is timestamped in microseconds as close to reception as
//! possible, then serialised to USB Serial/JTAG. `defmt` logs go to RTT: USB
//! carries the binary capture stream only.

use defmt_rtt as _;
use esp_backtrace as _;

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use esp_hal::{
    clock::CpuClock,
    timer::timg::TimerGroup,
    twai::{filter::SingleStandardFilter, TwaiConfiguration, TwaiMode},
    usb_serial_jtag::UsbSerialJtag,
};
use static_cell::StaticCell;

use esp32_s3::capture::{can_reader, usb_sink, TimestampedFrame, CAPTURE_DEPTH};
use esp32_s3::conf::{N2K_BITRATE, N2K_BITRATE_BPS};

esp_bootloader_esp_idf::esp_app_desc!();

static CAPTURE_CHANNEL: StaticCell<
    Channel<CriticalSectionRawMutex, TimestampedFrame, CAPTURE_DEPTH>,
> = StaticCell::new();

/// Accept everything. Set explicitly rather than relying on the esp-hal default.
/// A *standard* don't-care filter lets both extended and standard frames
/// through; a `SingleExtendedFilter` would drop standard frames.
const ACCEPT_ALL: SingleStandardFilter =
    SingleStandardFilter::new(b"xxxxxxxxxxx", b"x", [b"xxxxxxxx", b"xxxxxxxx"]);

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let mut config = TwaiConfiguration::new(
        peripherals.TWAI0,
        peripherals.GPIO41,
        peripherals.GPIO42,
        N2K_BITRATE,
        TwaiMode::ListenOnly,
    )
    .into_async();
    config.set_filter(ACCEPT_ALL);
    let can = config.start();

    let (_usb_rx, usb_tx) = UsbSerialJtag::new(peripherals.USB_DEVICE)
        .into_async()
        .split();

    let channel: &'static _ = CAPTURE_CHANNEL.init_with(Channel::new);

    defmt::info!(
        "N2K sniffer up: 250 kbps, listen-only, {} frame channel",
        CAPTURE_DEPTH
    );

    spawner.must_spawn(can_reader(can, channel.sender()));
    spawner.must_spawn(usb_sink(usb_tx, channel.receiver(), N2K_BITRATE_BPS));
}
