# korri-n2k-examples

Working nodes built on [korri-n2k](https://github.com/fard-draf/korri-n2k), one per architecture.

Each target is its own cargo project. There is no workspace.
Open the folder of your board, then build from there.

## Address claiming demo

The demo shows address claiming and live CAN traffic on my NMEA 2000 test bench.

https://github.com/user-attachments/assets/f74c1c86-3d74-44bd-8c03-438b41c576d6

*All devices have the same preferred address: 80, except Device 4, which is configured as listen-only.*
  * Device 2 has the lowest ISO NAME, so it wins the address claim and keeps its preferred address (80).
  * Device 3 has a fixed-address strategy and a higher ISO NAME than Device 2. It loses the address claim and cannot claim another address.
  * Device 1 also has a higher ISO NAME than Device 2, but uses an arbitrary-address strategy. After losing the claim for address 80, it successfully claims the next available address (81).

## Layout

| Path | What it holds |
|---|---|
| `shared-core/` | The ISO identities, the PGN generators, and the publishing loop. Shared by every target. |
| `arm/stm32/g431-cbu6/` | STM32G431 on Cortex-M4, embassy-stm32 FDCAN. |
| `xtensa/esp32-s3/` | ESP32-S3, esp-hal TWAI. Also holds the bus sniffer. |
| `risc-v/esp32-c3/` | ESP32-C3, esp-hal TWAI. |
| `linux/socketcan/` | Linux node on SocketCAN, tokio runtime. |

## Inside an embedded target

Every board folder follows the same five files.

| File | Role |
|---|---|
| `src/conf.rs` | Bitrate, buffer depths, report periods. |
| `src/starter.rs` | Clocks, pins, CAN controller. Board setup only. |
| `src/ports.rs` | The `CanBus` and `KorriTimer` drivers for this chip. |
| `src/app.rs` | Builds the ISO NAME and the address manager. |
| `src/manager_service.rs` | Static channels, the handle, and the runner task. |

`src/tasks/` only wraps `shared-core` in `#[embassy_executor::task]`.
The wrapper is needed because an embassy task cannot be generic.

## Sharing across runtimes

`shared-core` is split so the boards and the Linux node run the same code.

| Module | Runtime | What it holds |
|---|---|---|
| `instances` | none | The ISO identities and their claim strategy. |
| `samples` | none | One generator per PGN. No sleeping, no sending, no logging. |
| `publisher` | one per runtime | The loop: wait a period, generate, send. |
| `format` | embassy only | Actisense framing, for a board acting as a gateway. Unused. |

Only `publisher` knows about a runtime, because the two `AddressHandle` types
differ. Pick it with a feature: `embassy` by default, `tokio` for Linux.
The library refuses both at once, so `shared-core` does too.

Add a PGN by writing one struct in `samples.rs` and implementing `Sample`.
Every target can then publish it with `publish(handle, YourSample::new())`.

## Identities

`shared-core/src/instances.rs` holds the identities. Each one carries a NAME and
an address claim strategy, so a binary picks both by importing one constant.

| Constant | Strategy | Behaviour |
|---|---|---|
| `IDENTITY_1`, `IDENTITY_2` | `Fixed` | Address 80 only. Cannot Claim once a stronger NAME takes it. |
| `IDENTITY_3` to `IDENTITY_5` | `Arbitrary` | Start at 80, then walk the bus for a free address. |
| `IDENTITY_FIXED` | `Fixed` | Same as `IDENTITY_1`. Spare, no binary uses it. |
| `IDENTITY_LIST` | `SelfConfigurable` | Tries 80, 81, 82, then gives up. Spare, no binary uses it. |

All five ask for address 80, so any two of them collide.
The lowest NAME wins, and only `unique_number` varies, so the order is simple.
`IDENTITY_1` beats everyone, `IDENTITY_5` loses to everyone.

The strategy decides what the loser does. `Arbitrary` walks the bus for another
address. `Fixed` has nowhere to go: it goes Cannot Claim and falls silent.

The Arbitrary Address Capable bit of the NAME is derived from the strategy.
The library refuses a NAME and a strategy that disagree, so it is not a free field.
`IsoIdentity::is_arbitrary_address_capable` is the single place that decides it.

## Binaries

The same set is available on the three boards.

| Binary | Identity | Purpose |
|---|---|---|
| `simple` | none | Sends a raw CAN frame. Checks your wiring, no korri-n2k. |
| `dual_run_1` | 1 | Node A of the conflict pair. |
| `dual_run_2` | 2 | Node B, same address, weaker NAME. Ends in Cannot Claim. |
| `fast_packet` | 3 | One Fast Packet PGN only. |
| `stress_all` | 5 | Every PGN of `shared-core` at once. |
| `total` | 1 | Four PGNs, the everyday example. |

Flash `dual_run_1` and `dual_run_2` on two boards to watch a real address conflict.
Both are `Fixed`, so neither can move. Node A keeps 80. Node B goes Cannot Claim
and falls silent, every time. That state is the point of the pair.

The ESP32-S3 adds three more. None of them claims an address.

| Binary | Purpose |
|---|---|
| `sniffer` | Streams the raw bus over USB CDC. See `tools/`. |
| `receiver` | Listens only, and reassembles Fast Packet messages. |
| `claim_watcher` | Follows the address claiming, node by node. |

`claim_watcher` prints every Address Claim and message request as it happens.
It explains each field, decodes the 64-bit ISO NAME device identity, and says why
one device wins a contested address. A bus report follows every 10 seconds:
frame rate, errors, claims, conflicts, and the device table.
A node silent for a minute moves to a history section, so the live table stays true.

Flash it on a third board to watch `dual_run_1` and `dual_run_2` fight.

## Build and flash

Each target ships a `Justfile`.

```sh
cd xtensa/esp32-s3
just build total     # build one binary
just run dual_run_1  # flash it, then show the RTT logs
just fmt             # cargo fmt and clippy
```

Only the two ESP folders carry a `rust-toolchain.toml`.
The ESP32-S3 needs the `esp` toolchain from `espup`. Everything else is stable.

## Linux node

```sh
cd linux/socketcan
sudo ip link set can0 down
sudo ip link set can0 type can bitrate 250000
sudo ip link set can0 up
cargo run -- can0 3
```

It claims an address with `IDENTITY_3`, publishes position, depth and speed, and
prints what it hears. Same identities and same generators as the boards, so it
joins their bus as one more node. Pass `1` to `5` after the interface to select
the matching identity from `shared-core`; identity 3 is the default. Use it to
test without flashing anything.

Use `vcan0` only for a Linux-only simulation; a virtual interface cannot reach
the physical bus observed by `claim_watcher`.

`fast_packet` carries `IDENTITY_3` too. Do not run both on one bus.
`IDENTITY_4` is free if you want a fourth NAME.

## Capture tools

`xtensa/esp32-s3/tools/decode_capture.py` decodes a `sniffer` capture.
It reports frame counts, gaps, and Fast Packet integrity.

```sh
just capture capture.bin   # record from /dev/ttyACM0
just decode capture.bin    # decode and print the verdict
```

## Dependency

All targets pull korri-n2k 0.7 from crates.io. The address-status demo requires 0.7.1.

```toml
korri-n2k = { version = "0.7.1" }
```

Embedded targets enable the `embassy` feature, the Linux one enables `tokio`.
Neither is on by default, so one of them must be picked.

## Embassy versions

`embassy-time` 0.5 and `embassy-sync` 0.6 everywhere.
The executor and HAL layer change per target.

| Target | Executor and HAL |
|---|---|
| STM32G431 | `embassy-executor` 0.9, `embassy-stm32` 0.6 |
| ESP32-C3 and ESP32-S3 | `embassy-executor` 0.7, `esp-hal-embassy` 0.9 |

Keep `embassy-time` and `embassy-sync` aligned with `shared-core` when you bump a crate.
Types will not match at the API boundary otherwise.

## CAN wiring

| Board | RX | TX |
|---|---|---|
| STM32G431 | PA11 | PA12 |
| ESP32-S3 | GPIO41 | GPIO42 |
| ESP32-C3 | GPIO10 | GPIO9 |

All run at 250 kbit/s, the NMEA 2000 bitrate.
