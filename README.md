# mainichikatarenshu

> 毎日型練習して！

> Every day a martial arts form training!

[![Continuous Integration](https://github.com/paazmaya/mainichikatarenshu/actions/workflows/rust-ci.yaml/badge.svg)](https://github.com/paazmaya/mainichikatarenshu/actions/workflows/rust-ci.yaml)

E-ink display showing a kata for each day, along with current electricity price, using ESP-32 and written in Rust language.

## Project Status

**Current phase: experimental; the firmware builds but has not been verified on a CrowPanel.**

The firmware targets `xtensa-esp32s3-none-elf` using `esp-hal` rather than
ESP-IDF. It connects the display, the five buttons, the serial console, the
Helsinki schedule and the flash-backed training queue. The optional `wifi`
feature adds a station-mode Wi-Fi stack with SNTP time synchronization and
authenticated HTTPS (TLS 1.2/1.3 via `mbedtls-rs`) for 15-minute spot
electricity price retrieval and Supabase record upload.

Known limitations:

- **SNTP time is unauthenticated:** TLS certificate validity dates (`notBefore` /
  `notAfter`) are enforced and fail closed when no clock is set, but initial
  network time comes from unauthenticated UDP SNTP (unless set over UART with
  `TIME`). An active network attacker able to spoof NTP responses could shift
  the clock (`>= 2000-01-01`) to replay an expired certificate.
- **Embedded CA rotation requires reflashing:** Trusted root CAs are compiled
  into the firmware image from `certs/ca-roots.pem` (or `TLS_CA_BUNDLE`);
  updating trust anchors requires rebuilding and flashing the device.
- **At-least-once cloud delivery:** A pending record is acknowledged locally
  only after a complete `2xx` response is received. If Supabase commits an
  insert and the connection drops before the response arrives, the firmware
  retries the same record; add a `UNIQUE (date)` constraint on the Supabase
  table if duplicate rows must be prevented.
- Only the CONF button changes the workflow; MENU and EXIT refresh the display
  and UP/DOWN are only logged.
- Resting before 07:00 pauses the main loop for up to 60 seconds after a
  redraw; the chip does not enter deep sleep.
- Host tests (including local TLS handshake fixtures) and linked ESP32-S3
  release builds pass, but no path has yet been exercised on a physical
  CrowPanel or a live access point.

The existing `kata.yaml` maps categories to a kata name. A category may also
contain a list of names:

```yaml
karate: "First kata"
kobujutsu:
  - "Second kata"
  - "Third kata"
```

All categories contribute to one selection pool; category labels are not
stored in training records. Each category and name must be nonempty, and names
must be at most 64 UTF-8 bytes. The build rejects invalid files and duplicate
categories. When the file is absent, a warning is emitted and no kata names
are embedded. Rebuild after changing the file. Set `KATA_CONFIG` to a
different YAML file path when building with an alternate list. If explicitly
set, that file must exist.

The optional `wifi` feature uses `esp-radio`, `esp-rtos`, `embassy-net` and
`mbedtls-rs` (MbedTLS 3.x compiled from C), including Espressif's precompiled
radio libraries; the firmware is **not** pure Rust throughout.
The bare-metal HAL still calls functions in the chip's existing ROM, and
`espflash` uses a bootloader; this is not a claim that the entire device
boot chain consists of Rust source.

Build the experimental firmware after installing the ESP Xtensa toolchain
(see [Toolchain setup](#toolchain-setup); `xtensa-esp32s3-elf-gcc` on `PATH` and
`LIBCLANG_PATH` pointing to `esp-clang/lib`, as set up by `export-esp.sh` /
`export-esp.ps1`) and Rust 1.95 or newer. Building with `--features wifi` (or host
`--features test-utils`) also requires `cmake` (>= 3.19) and `ninja` to compile
MbedTLS. The binary has `required-features = ["firmware"]`, so a plain
`cargo build` builds only the library:

```sh
cargo +esp build --release --features firmware
# Include the optional station-mode Wi-Fi + HTTPS stack:
cargo +esp build --release --features wifi
# Flash and open the serial monitor (runner: `espflash flash --monitor`):
cargo +esp run --release --features wifi
```

Linked release footprints on `xtensa-esp32s3-none-elf`:

- `firmware` (offline): ~170 KiB loadable flash, 64 KiB heap (`esp_alloc`).
- `wifi` (Wi-Fi + Embassy + MbedTLS 1.2/1.3 + root CAs): ~825 KiB loadable
  flash inside the 4 MiB `factory` partition, 160 KiB heap.

### Partition table and first flash

**Install the partition table before (or together with) the first firmware
flash.** The firmware keeps its state in a dedicated `training` partition and
refuses to start without it, panicking with
`Invalid or missing training partition: MissingTraining`. Unless told otherwise
`espflash` writes its own default table (`nvs`, `phy_init` and `factory`
only) at `0x8000` on every flash unless `--partition-table` is given. The
runner in `.cargo/config.toml` passes `--partition-table partitions.csv`, so
`cargo +esp run` installs the table for you; the panic appears when flashing
with a plain `espflash flash` instead.

1. **Back up the existing flash.** Installing a different partition table can
   make existing data inaccessible, so do not assume a device already has the
   proposed layout:

   ```sh
   espflash read-flash 0 0x800000 flash-backup.bin
   ```

2. **Check `partitions.csv`.** Edit it if needed and print the resulting table:

   ```sh
   espflash partition-table partitions.csv
   ```

   The output should list `nvs` (`0x9000`, 32 KiB), `phy_init` (`0x11000`,
   32 KiB), `factory` (`0x20000`, 4 MiB) and `training` (`0x420000`, 128 KiB, a
   `data` partition with the custom subtype the firmware validates).

3. **Flash the table together with the firmware.** `espflash` accepts the CSV
   directly and places the application at the `factory` offset. The runner
   (`espflash flash --monitor --partition-table partitions.csv`) does this on
   every run, so it overwrites the partition table each time; run it from the
   repository root so that `partitions.csv` is found:

   ```sh
   cargo +esp run --release --features wifi
   ```

   Without the runner, pass the same option to `espflash flash` yourself:

   ```sh
   espflash flash --monitor --partition-table partitions.csv target/xtensa-esp32s3-none-elf/release/mainichikatarenshu
   ```

### Experimental Wi-Fi setup

The `wifi` build accepts credentials over UART0 at 115200 baud. Send each
command as a separate newline-terminated line:

```text
SSID <network name>
PASSWORD <WPA2 passphrase>
CONNECT
```

SSID and passphrase are printable ASCII, with lengths of 1–32 and 8–63 bytes
respectively. Only WPA2-Personal is supported; no open-network or WPA3
provisioning is provided. `CONNECT` stores the credentials **in plaintext** in
the `training` flash partition and they are reused automatically after a
restart. The serial connection is plaintext, the terminal may echo or retain
commands, and RAM is not securely erased; use a trusted serial connection.
Restart to change credentials or retry a radio initialization failure.

Once connected, the firmware:

- sets the clock from `pool.ntp.org` (falling back to `time.cloudflare.com`)
  unless a valid clock is already set via `TIME` or preserved across deep sleep
  (the build-time default does not count),
  retrying failed queries every minute;
- requests the 15-minute spot price for the current UTC timestamp (`GET /v2/price.json?date=YYYY-MM-DDTHH:MM:SS.000Z`)
  from [porssisahko.net](https://porssisahko.net/api) over HTTPS (port 443) once
  per quarter-hour, retrying failures every minute;
- uploads the oldest pending record to Supabase over HTTPS (port 443) when
  configured and the clock is synchronized, backing off for 60 seconds after
  failures and acknowledging locally only after a complete `2xx` response.

HTTPS connections enforce X.509 chain validation (`AuthMode::Required`), TLS
SNI and DNS SAN hostname matching, certificate `notBefore` / `notAfter`
checks, hardware TRNG entropy (`RNG` + `ADC1`), 10-second TCP/DNS timeouts,
15-second TLS exchange timeouts, and 2 KiB response limits (`Content-Length`
and `Transfer-Encoding: chunked` are supported; unsupported transfer encodings
and truncated messages are rejected without plaintext fallback).

#### Root CA bundle and rotation

Default trust anchors live in `certs/ca-roots.pem` (Google Trust Services `R1`/`R4`,
GlobalSign Root CA, Let's Encrypt `ISRG Root X1`/`X2`, DigiCert Global Root
`G2`/`G3`, and SSL.com ECC Root). To rotate roots or trust a private CA (for
example a self-hosted Supabase instance), either update `certs/ca-roots.pem`
or pass an alternate PEM file at build time:

```sh
TLS_CA_BUNDLE=/path/to/custom-roots.pem cargo +esp build --release --features wifi
```

`build.rs` validates PEM framing at compile time and strips comment lines
before embedding the bundle.

#### Supabase configuration

Supabase is configured at build time. Synchronization is disabled unless both
host and key are set:

```sh
SUPABASE_HOST=xyz.supabase.co SUPABASE_ANON_KEY=<anon key> \
  SUPABASE_TABLE=training_records \
  cargo +esp build --release --features wifi
```

`SUPABASE_TABLE` defaults to `training_records`. The table receives
`{"date": "YYYY-MM-DD", "kata": "...", "confirmed_at": "<RFC 3339>" | null}`.
The key is embedded in the firmware image (and never printed in serial logs),
so only use a public anon key with row-level security that permits inserts
only, and define a `UNIQUE (date)` constraint if retried uploads should not
insert duplicate dates.

### Experimental offline operation

Install the partition table first, as described in
[Partition table and first flash](#partition-table-and-first-flash). The
proposed `partitions.csv` reserves 128 KiB at `0x420000` for training (after a
4 MiB `factory` app partition). The firmware refuses to use storage unless the
installed table has a valid, non-overlapping `training` partition with the
expected type and alignment.

On UART0 through the board's serial interface (115200 baud), send newline
terminated commands:

- `TIME <UTC unix seconds>` sets the clock, for example `TIME 1767225600`.
  After power loss or a normal reset the clock restarts from the firmware
  build time (an estimate, printed at startup), so the device runs without any
  setup; send `TIME` or let the `wifi` build synchronize over SNTP to correct
  it. Until then Supabase uploads are held back, TLS certificate dates are
  still not trusted (HTTPS fails closed), and records created on the estimated
  clock may carry the wrong date. If the time is moved backwards, the training workflow
  pauses until the clock reaches the last observed time or is corrected. After
  restart, the comparison uses the time saved at the last state change.
- `LIST` prints pending completed-day records in order.
- `ACK <YYYY-MM-DD>` permanently removes the oldest pending record. Only
  acknowledge it after independently saving or synchronizing it. An
  out-of-order date is rejected.

The queue holds 32 completed days. When full, the active record remains
preserved and the workflow pauses; export and acknowledge the oldest record
to resume. The display is refreshed on state changes, not continuously, so
the time shown is the time of the last refresh.

The storage implementation uses `sequential-storage` snapshots. While a
simulated interrupted write and failed commit are tested, the library can skip
corrupt items and perform automatic repair. Arbitrary corruption, all power-loss
points and real flash behavior have not been validated; silent rollback or data
loss cannot yet be ruled out. A flash write error stops the firmware because
the outcome may be ambiguous. Do not rely on this experimental queue as the
only copy of important records.

Do not flash an existing device without first checking its flash layout and
backing up its data. The earlier ESP-IDF-based work (bootloader, debug symbols,
driver search) is documented in [DEVELOPMENT_JOURNEY.md](DEVELOPMENT_JOURNEY.md)
and no longer applies to the current build.

### Source layout

| Path                                                                  | Purpose                                                               |
| --------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `src/main.rs`                                                         | Firmware entry point: peripheral setup and the main loop              |
| `src/wireless.rs`                                                     | Wi-Fi station, SNTP, price and Supabase clients (`wifi` feature)      |
| `src/training.rs`                                                     | Daily training state machine and kata selection                       |
| `src/schedule.rs`                                                     | Helsinki 07:00–23:00 schedule with DST handling                       |
| `src/storage.rs`                                                      | Versioned flash snapshots of training state and Wi-Fi credentials     |
| `src/partitions.rs`                                                   | Partition table validation for the `training` partition               |
| `src/console.rs`                                                      | Serial line buffering and command parsing                             |
| `src/clock.rs`                                                        | RTC timestamp validation and conversion                               |
| `src/buttons.rs`                                                      | Button debouncing                                                     |
| `src/screens.rs`                                                      | Screen selection and application screens                              |
| `src/layout.rs`, `src/text.rs`                                        | Drawing helpers, text presets, wrapping and alignment                 |
| `src/tls.rs`                                                          | Authenticated MbedTLS 1.2/1.3 client and fail-closed `WallClock`      |
| `src/electricity.rs`, `src/supabase.rs`, `src/sntp.rs`, `src/http.rs` | Spot price, Supabase sync, SNTP and HTTP/1.x framing helpers          |
| `src/network_config.rs`                                               | Wi-Fi credential validation                                           |
| `certs/ca-roots.pem`                                                  | Default WebPKI root CA bundle for HTTPS                               |
| `crates/crowpanel-ssd1680`                                            | SSD1680 e-paper driver ([README](crates/crowpanel-ssd1680/README.md)) |
| `build.rs`, `build_support/`                                          | Validates and embeds `kata.yaml`, Supabase settings and CA roots      |

Unlike `crates/crowpanel-ssd1680` (which isolates hardware-neutral panel
commands), `src/electricity.rs` is kept as an internal library module rather
than a separate crate until a second consumer exists, because its formatting
and 15-minute slot tracking are tailored to this firmware's display workflow.

### Daily schedule

Each day at 07:00 Helsinki time the e-ink screen updates with a kata name chosen
at random from the configured list. At 23:00 Helsinki time the day is recorded,
as incomplete if the CONF button has not been pressed before that. Deep sleep
between these times is not implemented yet (see the roadmap).

The former Google Drive plan has been replaced by the Supabase sync.

### Toolchain setup

The `esp` toolchain pinned in `rust-toolchain.toml` is installed with
[espup](https://github.com/esp-rs/espup); flashing uses
[espflash](https://github.com/esp-rs/espflash) (v4.x). Host tests and host
clippy use the `stable` toolchain. Large files under `info/` and `logo.png` are
stored with Git LFS.

```sh
cargo install espup@0.11.0 # Version that still works in Windows: could not be opened: LoadLibraryExW failed
espup install # Install ESP tools
cargo install espflash
```

Might need to set the tools available in Windows:

```ps1
& ~\export-esp.ps1
```

In case there was an earlier installation of the esp toolchain, remove its
folder (on Windows `%USERPROFILE%\.rustup\toolchains\esp`) so it is not reused.
On Windows, LLVM is also needed (`choco install llvm`,
[package](https://community.chocolatey.org/packages/llvm)).

## Parts

[CrowPanel ESP32 2.9" E-paper HMI Display with 128\*296 Resolution, Black/White Color Driven By SPI Interface](https://www.elecrow.com/crowpanel-esp32-2-9-e-paper-hmi-display-with-128-296-resolution-black-white-color-driven-by-spi-interface.html)

This has internal ESP32-S3 chip, packaged with the E-paper display.

The "info" folder contains datasheets and further details.

### Features

- 2.9-inch E-Paper display, 128\*296 resolution, black and white, using SPI interface communication;
- ESP32-S3 as the main chip, frequency up to 240MHz;
- Support full viewing angle, clearly visible from any angle;
- High contrast and high reflectivity， can provide better visual performance;
- Pure reflection mode, no backlight required, completely relying on light reflection to display content, and the displayed content will not be lost even if the power is off;
- Hard-coated anti-glare display surface, it can keep the content clearly visible even under direct sunlight;
- Support Arduino IDE, ESP IDF, and MicroPython development environment to get a smooth development experience；
- Ultra-low power consumption and partial refresh function, significantly saving energy;
- Rich buttons and interfaces (such as GPIO interface, UART interface, home button, etc.) for easy development and operation.

| 2.9-inch Display Port | Pin Number                                                          |
| --------------------- | ------------------------------------------------------------------- |
| MENU Button           | IO2                                                                 |
| Rotary Switch         | Down(IO4); Up(IO6); CONF(IO5)                                       |
| EXIT Button           | IO1                                                                 |
| GPIO                  | IO3; IO9; IO15; IO17; IO19; IO21; IO8; IO14; IO16; IO18; IO20; IO38 |
| SD Card Slot(SPI)     | MOSI(IO40); MISO(IO13); CLK(IO39); CS(IO10)                         |

```sh
cargo espflash board-info

Chip type:         esp32s3 (revision v0.2)
Crystal frequency: 40 MHz
Flash size:        8MB
Features:          WiFi, BLE
```

## Development Roadmap

### Phase 1: Display Driver ✅ COMPLETE

- [x] Custom SSD1680 driver implementation
- [x] SPI communication and pin configuration
- [x] Display initialization matching Arduino reference
- [x] Text rendering with `embedded-graphics`
- [x] Image display with build-time conversion
- [x] RTC date/time integration

### Phase 2: Core Application Features 🚧 IN PROGRESS

- [x] **Kata Management**
  - [x] Define kata list via `kata.yaml` embedded at build time
  - [x] Random kata selection algorithm with RNG
  - [x] Daily kata rotation state machine (`src/training.rs`)
  - [x] Helsinki timezone and daylight savings schedule (`src/schedule.rs`)
- [x] **Partition Validation & Flash Persistence**
  - [x] Partition table structure and checksum validation (`src/partitions.rs`)
  - [x] Sequential flash snapshots with postcard serialization (`src/storage.rs`)
- [x] **Button Input Handling**
  - [x] Monotonic debouncer for active-low buttons (`src/buttons.rs`)
  - [x] CONF button (GPIO5) confirm training
  - [x] MENU button (IO2) input handling
  - [x] EXIT button (IO1) input handling
  - [x] Rotary switch Down (IO4) and Up (IO6) inputs (`ButtonBank`)
- [ ] **Power Management & Low Power**
  - [x] SSD1680 display deep sleep command (`sleep()`)
  - [x] Schedule-aware night rest pause (up to 60s loop pause before 07:00 Helsinki)
  - [ ] ESP32-S3 RTC deep sleep until 07:00 Helsinki alarm
  - [ ] GPIO button wake-up support (ext0/ext1 wakeup on CONF button)

### Phase 3: Connectivity & Data Sync 🚧 IN PROGRESS (awaiting hardware verification)

- [x] **WiFi & Time Integration**
  - [x] Station-mode connection via `esp-radio` and `embassy-net` (`src/wireless.rs`)
  - [x] Automatic SNTP / NTP network time synchronization (`src/sntp.rs`)
  - [x] Persistent Wi-Fi credentials in flash storage (`src/storage.rs`)
- [x] **Electricity Price Display**
  - [x] Price shown on the kata reminder screen (`src/screens.rs`)
  - [x] 15-minute spot price client and JSON parser (`src/electricity.rs` & `src/wireless.rs`)
  - [x] Authenticated HTTPS (TLS 1.2/1.3, CA chain, SAN & date validation in `src/tls.rs`)
- [x] **Remote Synchronization (Supabase)**
  - [x] Local training record queue (up to 32 days in flash)
  - [x] UART manual export and acknowledgement (`LIST`, `ACK`)
  - [x] HTTPS/REST client and retry-safe sync coordinator (`src/supabase.rs` & `src/wireless.rs`)
  - [x] Automatic record acknowledgement on successful sync (`src/main.rs`)
  - [x] Authenticated HTTPS (TLS 1.2/1.3, CA chain, SAN & date validation in `src/tls.rs`)
- [ ] **Physical Device Validation**
  - [ ] End-to-end Wi-Fi, SNTP, live HTTPS price fetch, and Supabase upload on a CrowPanel ESP32-S3

### Phase 4: Enhancements 🎯 FUTURE

- [ ] Weather integration for outdoor training
- [ ] Battery optimization and monitoring
- [ ] OTA (Over-The-Air) firmware updates
- [ ] Web interface for configuration
- [ ] Training statistics and streaks
- [ ] Multiple kata difficulty levels

## Testing

All hardware-independent logic lives in the library crate and is tested on
the host with the `test-utils` feature, which builds the library with `std`.
The firmware binary only wires peripherals to that logic, so it is checked by
compiling it for the device.

| Location                                                             | Kind                                                              |
| -------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `#[cfg(test)]` modules in `src/` and `crates/crowpanel-ssd1680/src/` | Unit tests (including in-memory TLS 1.2/1.3 certificate fixtures) |
| `tests/state_storage.rs`                                             | Flash persistence with a simulated, failure-injecting flash       |
| `tests/kata_config_tests.rs`                                         | Validates repository `kata.yaml` and `certs/ca-roots.pem`         |
| Doc comments                                                         | Documentation tests                                               |

Rendering tests draw into a `Display2in9` frame buffer and compare or inspect
its bytes, so no panel is required.

```sh
# Unit and integration tests, including the driver crate
cargo +stable test --workspace --lib --tests --features test-utils --target <host-triple>

# Documentation tests
cargo +stable test --workspace --doc --features test-utils --target <host-triple>

# Lints
cargo +stable clippy --workspace --lib --tests --features test-utils --target <host-triple> -- -D warnings
cargo +esp clippy --release --features wifi -- -D warnings

# Applying automatic Clippy fixes
# On host targets (unit tests, integration tests, and library):
cargo +stable clippy --fix --all-targets --features test-utils --target <host-triple> --allow-no-vcs
# On firmware target (device binaries & library):
cargo clippy --fix --allow-no-vcs --features firmware

# Coverage - needs `cargo install cargo-tarpaulin`; settings in tarpaulin.toml, fails below 90%
cargo +stable tarpaulin

# Formatting
cargo fmt --all -- --check
```

The host triple is, for example, `x86_64-unknown-linux-gnu` or
`aarch64-apple-darwin` (you can dynamically pass `$(rustc -vV | sed -n 's/host: //p')`);
it is needed because `.cargo/config.toml` defaults to the ESP32-S3 target
`xtensa-esp32s3-none-elf` (note: unset `LIBCLANG_PATH` on host runs if
`export-esp.sh` pointed it at `esp-clang`).

> **Note on `--all-targets` and bare-metal ESP32:**
> Running `cargo clippy --all-targets` or `cargo clippy --fix --all-targets` without specifying
> `--target <host-triple>` fails with `error[E0463]: can't find crate for std`.
> Because `.cargo/config.toml` defaults to the bare-metal `no_std` target `xtensa-esp32s3-none-elf`
> (`build-std = ["core", "alloc"]`), passing `--all-targets` causes Cargo to try building test harnesses
> and host integration tests (`tests/*.rs`) for the bare-metal microcontroller target, which has no
> standard library (`std`). For this reason, `.vscode/settings.json` configures
> `"rust-analyzer.check.allTargets": false`. Always provide `--target <host-triple>` when running
> tests or linting test targets, or lint firmware targets without `--all-targets`.

To conserve CI minutes, GitHub Actions CI runs the firmware release builds for both feature sets, firmware clippy, host unit and integration tests (`--lib --tests`), formatting, and host clippy; documentation tests (`--doc`) and `cargo +stable tarpaulin` are local checks.

On Windows the host test build fails in `mbedtls-rs-sys` with the MSVC toolchain (MbedTLS headers use GCC attributes); run the host tests on Linux, WSL or the Dev Container, as CI does.

### Repeatable hardware verification steps (when a CrowPanel and test services are available)

1. **Inspect and back up flash** (`espflash read-flash`), then install `partitions.csv` as described in [Partition table and first flash](#partition-table-and-first-flash) before anything else is flashed.
2. **Build and flash a test image** (with `--partition-table partitions.csv`) pointing `SUPABASE_HOST`, `SUPABASE_ANON_KEY` and `SUPABASE_TABLE` at a dedicated non-production Supabase table (with RLS insert-only policy and `UNIQUE (date)`), then open the UART0 monitor at 115200 baud.
3. **Verify Wi-Fi + SNTP + HTTPS price fetch:** send `SSID <name>`, `PASSWORD <passphrase>`, `CONNECT`; confirm serial output logs `Wi-Fi IPv4: ...`, `SNTP sync success!`, and `Current electricity price: ... c/kWh`, and that the panel renders the kata and price.
4. **Verify manual `TIME` fallback:** on a network blocking UDP port 123, reboot, send `TIME <current UTC unix seconds>`, and confirm HTTPS price retrieval still succeeds using the manual clock.
5. **Verify Supabase upload and queue drain:** advance across a day boundary (or confirm a day and advance past 23:00 Helsinki time via `TIME`), run `LIST` to see the queued record, then confirm `Supabase sync succeeded for record ...` appears and `LIST` drops to `Pending records: 0`.
6. **Verify failure and reconnect paths:** power-cycle the access point to confirm automatic Wi-Fi reconnection after 5s, and build a test image with `TLS_CA_BUNDLE` set to an unrelated test CA to confirm both HTTPS clients fail closed and leave pending records intact in `LIST`.

## Build System

The build script (`build.rs`) generates four files in `OUT_DIR`:

- `kata.rs` - the kata names from `kata.yaml` (or `KATA_CONFIG`), validated by
  `build_support/kata_config.rs`;
- `supabase.rs` - the optional Supabase settings from `SUPABASE_HOST`,
  `SUPABASE_ANON_KEY` and `SUPABASE_TABLE`;
- `ca_bundle.rs` - the validated PEM root CA bundle from `certs/ca-roots.pem`
  (or `TLS_CA_BUNDLE`), validated by `build_support/ca_config.rs`;
- `build_time.rs` - the build time in Unix seconds (`clock::BUILD_UNIX_SECS`),
  the clock's starting point after a reset. It is refreshed whenever `src/`,
  `crates/`, `Cargo.toml` or `Cargo.lock` change, and `SOURCE_DATE_EPOCH`
  overrides it for reproducible builds.

## License

MIT

## Dev Containers

This repository offers Dev Containers supports for:

- [VS Code Dev Containers](https://code.visualstudio.com/docs/remote/containers#_quick-start-open-an-existing-folder-in-a-container)
- [GitHub Codespaces](https://docs.github.com/en/codespaces/developing-in-codespaces/creating-a-codespace)
  > **Note**
  >
  > In [order to use GitHub Codespaces](https://github.com/features/codespaces#faq)
  > the project needs to be published in a GitHub repository and the user needs
  > to be part of the Codespaces beta or have the project under an organization.

If using VS Code or GitHub Codespaces, you can pull the image instead of building it
from the Dockerfile by selecting the `image` property instead of `build` in
`.devcontainer/devcontainer.json`. Further customization of the Dev Container can
be achived, see [.devcontainer.json reference](https://code.visualstudio.com/docs/remote/devcontainerjson-reference).

When using Dev Containers, some tooling to facilitate building, flashing and
simulating in Wokwi is also added.

## Wokwi Simulation

### VS Code Dev Containers and GitHub Codespaces

The Dev Container includes the Wokwi Vs Code installed, hence you can simulate your built projects doing the following:

1. Press `F1`
2. Run `Wokwi: Start Simulator`

> **Note**
>
> We assume that the project is built in `debug` mode (`cargo +esp build --features firmware`, or `--features wifi`), if you want to simulate projects in release, please update the `elf` and `firmware` proprieties in `wokwi.toml`.

For more information and details on how to use the Wokwi extension, see [Getting Started] and [Debugging your code] Chapter of the Wokwi documentation.

[Getting Started]: https://docs.wokwi.com/vscode/getting-started
[Debugging your code]: https://docs.wokwi.com/vscode/debugging
