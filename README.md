# korri-n2k-examples

Example firmwares for the [korri-n2k](https://crates.io/crates/korri-n2k) NMEA 2000 library, targeting several architectures.

Every board shares the same logic: a set of binaries that can be flashed with different ISONAMEs to observe network management (ISO Address Claim, ISO Request, etc.) on a real backbone.

## Binaries

| Binary | Purpose |
|---|---|
| `simple` | Sanity check of your hardware setup — no korri-n2k dependency |
| `dual_run_1` / `dual_run_2` | Two ISONAME instances, used to trigger address conflicts |
| `fast_packet` | Fast Packet PGNs only |
| `stress_all` | Sends every PGN from `shared-core` to saturate the backbone |
| `total` | Full-featured example |

## Embassy versions

All targets are built against **`embassy-time` 0.5** and **`embassy-sync` 0.6** — the versions `shared-core` (and `korri-n2k`) depend on. The executor/HAL layer varies per target:

| Target | Executor / HAL |
|---|---|
| STM32G431 | `embassy-executor` 0.9, `embassy-stm32` 0.6 |
| ESP32-C3 / ESP32-S3 | `embassy-executor` 0.7, `esp-hal-embassy` 0.9 |

If you bump an embassy crate, keep `embassy-time`/`embassy-sync` aligned across `shared-core` and every target, otherwise types won't match at the API boundary.

## Layout

- **`shared-core/`** — PGN definitions shared across all targets (heartbeat, position, depth, engine, AIS, ...). Architecture-agnostic: add your own PGNs by following the existing structure.
- **`arm/stm32/g431-cbu6/`** — STM32G431 (Cortex-M4)
- **`linux/socketcan/`** — Linux SocketCAN (WIP)
- **`risc-v/esp32-c3/`** — ESP32-C3 (WIP)
- **`xtensa/esp32-s3/`** — ESP32-S3 (WIP)
