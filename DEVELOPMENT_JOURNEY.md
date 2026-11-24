# Building a Rust E-Paper Display Driver: A Development Journey

> From Arduino examples to a working Rust driver for the SSD1680 E-Paper display on ESP32-S3

**Project:** [mainichikatarenshu](https://github.com/paazmaya/mainichikatarenshu) - Daily kata training reminder using E-ink display  
**Hardware:** CrowPanel ESP32 2.9" E-paper HMI Display (128×296, SSD1680 driver)  
**Language:** Bare-metal Rust (`no_std`) with `esp-hal` & Embassy (previously ESP-IDF)

## Introduction: The Vision

The goal was simple yet ambitious: create a daily martial arts kata reminder using an E-ink display powered by ESP32-S3, written entirely in Rust. The display would wake up at 7:00 AM, show a random kata name, and track whether the training was completed. The data would sync to Google Drive for long-term tracking.

**SD Prompt:** `A minimalist e-paper display showing Japanese kanji characters for martial arts kata, mounted on a wooden desk with morning sunlight, black and white aesthetic, clean modern design, photorealistic, 4k`

## Chapter 1: The Toolchain Odyssey (January–February 2025)

### Setting Up the ESP32-S3 Environment

The journey began with setting up the Rust toolchain for ESP32 development. The [CrowPanel ESP32 2.9" E-paper display](https://www.elecrow.com/crowpanel-esp32-2-9-e-paper-hmi-display-with-128-296-resolution-black-white-color-driven-by-spi-interface.html) features an integrated ESP32-S3 chip (240MHz, 8MB flash) with a 128×296 resolution black-and-white E-paper display.

```bash
cargo install espup@0.11.0
espup install
```

The first challenge emerged immediately: Windows compatibility issues with the ESP toolchain. The solution required removing old installations from `C:\Users\[User]\.rustup\toolchains\esp\` and installing LLVM via Chocolatey:

```bash
choco install llvm
```

### The Bootloader Battle

For an entire day, I fought with bootloaders. The device came with `ESP-IDF v5.1-beta1` bootloader, but I needed to update it. After installing ESP-IDF v5.4 and creating a custom bootloader:

```bash
idf.py create-project kukkuu
idf.py menuconfig
idf.py bootloader
```

The twist? `espflash` has its own bootloader that it provides when not using the `esp-idf-sys` package. The solution was to explicitly specify the custom bootloader in `.cargo/config.toml`:

```toml
runner = "espflash flash --monitor --bootloader kukkuu/build/bootloader/bootloader.bin --partition-table partition-table.bin"
```

### Silent Failures: Old Bootloader, Old Partition Table, No Symbols

Three more dead ends followed (commits `d4ca374`, `ecb5429`, `4fbcf78`, `e6117ba`):

- After flashing a new bootloader, the boot log still printed the old `ESP-IDF v5.1-beta1` banner. A full `erase-flash` then showed that an older partition table had been in use all along.
- The firmware aborted at runtime, but the ELF had no debug symbols in either profile, so there was nothing to debug with. `readelf -S` showed the same four section headers for dev and release. The ESP-IDF template then failed to link (`ldproxy` panic) until a one-line `build.rs` calling `embuild::espidf::sysenv::output()` was added; only then did `.debug_*` sections appear.
- Version bumps broke things: `esp-idf-svc` 0.51 no longer compiled, so it was held at 0.50.1 and excluded from Renovate (`ec38e25`).

**Learning:**

- Check which bootloader and partition table are really on the device (boot log banner, `erase-flash`) before changing either.
- Inspect the artifact (`readelf -S`) instead of assuming a profile setting produced symbols; a missing `build.rs` hook failed silently.
- Pin tightly-coupled ecosystem crates and exclude them from automatic update PRs until the set is known to work together (the same reason `esp-radio` is pinned with `=` in `Cargo.toml` today).
- A tool's defaults (`espflash` bringing its own bootloader) can override what the project thinks it configured.

**SD Prompt:** `A computer screen showing terminal windows with compilation logs, ESP32 development board connected via USB cable, dim workshop lighting, technical atmosphere, developer workspace, cinematic lighting, detailed electronics`

## Chapter 2: The Driver Search (October 14, 2025 - Morning)

### Existing Drivers Don't Fit

Initially, I tried using existing Rust drivers:

- **epd-waveshare**: The `epd2in9_v2` module worked partially, but commands didn't match
- **embedded-graphics**: Integration was complex, colors weren't mapping correctly
- **mbv/ssd1680**: Promising but commands didn't align with the Elecrow Arduino examples

The display uses an SSD1680 controller, but the command sequences in the [working Arduino examples](https://github.com/Elecrow-RD/CrowPanel-ESP32-E-Paper) differed from all existing Rust drivers. The decision was made: **write a custom driver from scratch**.

### The Detours That Led There (January–February 2025)

The history shows that the decision was not instant. Roughly in order:

1. **`epd-waveshare` 0.6 with `embedded-graphics`** (`19017a7`, `d6e5283`): it compiled only after dropping to 0.5.0 and diffing the two releases. Text then appeared, but colors had to be `epd_waveshare::color::Color` rather than `BinaryColor`, and each draw landed on top of the previous one because the screen was never cleared.
2. **Other board configurations of the same crate** (`dde3156`): `epd2in9_v2` was the only module that did anything, so similar boards were tried at random. With a different driver library it became "hard to know if anything is updated to the display really".
3. **Vendoring `mbv/ssd1680`** (`ebc5099`, `607bdbc`): the code compiled and ran, but still did not clear or write.
4. **Restructuring for "more direct pin access"** (`f428efa`): the commit message says it plainly: _that did not help_.
5. **Plenty of logging** (`170587e`): added to find out why the code stalled after the activate command. This pointed at the BUSY pin (Chapter 4).
6. **Reading the specification more closely** (`61cedde`): the Arduino example's command bytes (`0xF4`, `0xB1`, `0x91`, `0xC7`, data entry mode, border waveform) were written down next to the datasheet, which prepared the line-by-line comparison in Chapter 3.

**Learning:**

- Identify the controller chip, not only the panel size. A crate for another panel with a similar size shares some commands and silently differs in others.
- "It compiles and runs" proves nothing for a display. Without a visible effect, every attempt was a coin flip, so a known-good reference (the Arduino code) was needed first.
- Moving code around is not debugging. The step that made progress was instrumenting the timing, not reorganizing the pins.
- Writing the reference values down in one place turns a vague mismatch into a diff.

### Building the Foundation

Starting from the `mbv/ssd1680` structure, I created a new driver in `src/ssd1680/` with modules:

- `cmd.rs` - Command definitions
- `color.rs` - Color handling
- `driver.rs` - Main driver logic
- `interface.rs` - SPI communication
- `graphics.rs` - Drawing primitives
- `pins.rs` - GPIO configuration

The driver needed to match the Arduino implementation exactly, translating C++ patterns to idiomatic Rust.

**SD Prompt:** `Split screen comparison showing Arduino C++ code on left and Rust code on right, syntax highlighting, dark theme IDE, code translation concept, technical documentation style, clean and professional`

## Chapter 3: Matching the Working Implementation (October 14, 2025 - Late Morning)

### Three Critical Discoveries

After comparing the working C++ implementation line-by-line with my Rust driver, I identified three critical differences documented in `DRIVER_UPDATES.md`:

#### 1. Display Update Control Value (0xF4 vs 0xC7)

The SSD1680 datasheet suggests `0xC7` for the Display Update Control 2 command, but the working Arduino code uses `0xF4`:

```rust
// Changed from datasheet value
// &[0xC7], // Standard full update value from SSD1680 datasheet
&[0xF4], // Value from working C++ implementation
```

#### 2. Data Inversion Required

The C++ implementation inverts all data bytes before sending to the display (`!image_bw[i]`). This is critical for correct black/white representation:

```rust
// Invert data to match working C++ implementation
log::info!("Inverting data for display");
let inverted: Vec<u8> = buffer.iter().map(|&b| !b).collect();
self.interface.cmd_with_data(Cmd::WRITE_BW_DATA, &inverted)?;
```

#### 3. Fast Update Method

Implemented a `fast_update()` method matching the C++ `epd_fast_update()` function for quicker refreshes with acceptable ghosting:

```rust
pub fn fast_update(&mut self, delay: &mut impl DelayNs) -> Result<(), DisplayError> {
    self.interface.reset()?;
    // Hardware reset sequence
    // Display update with 0xB1, 0x91, 0xC7
    // Temperature parameter: 0x64, 0x00
}
```

**Learning:** When a datasheet and a known-working implementation disagree, find the differences by comparing line by line, and record each one together with the reason it was changed. Three small differences (`0xF4`, inversion, update mode) explained everything the datasheet alone could not.

**SD Prompt:** `Close-up of e-paper display showing partial refresh in progress, ghosting effect visible, black and white pixels transitioning, macro photography, technical documentation style, high contrast`

## Chapter 4: The Blank Screen Mystery (October 14, 2025 - Noon)

### The Problem

After flashing the device with the updated driver, commands were being sent successfully—logs confirmed every step—but the display remained completely blank. No errors, no crashes, just... nothing.

### The Root Cause: Missing BUSY Pin Wait

E-paper displays are **slow**. The physical update process involves:

1. Applying voltages to change particle positions
2. Multiple refresh cycles
3. Takes 1-3 seconds depending on update mode

The bug was in the display update sequence:

```rust
// ❌ WRONG - Missing wait
self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
self.interface.data(&[0xF4])?;
self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
// Immediately continue without waiting ← BUG!
```

During the update, the BUSY pin goes HIGH. The code **must** wait for BUSY to go LOW before continuing, or subsequent commands will interrupt the update process.

### The Fix

Added a `wait_busy()` method and called it after every `MASTER_ACTIVATE` command:

```rust
// ✅ CORRECT - Wait for display update
self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
log::info!("Waiting for display update to complete (BUSY pin)...");
ssd1680.wait_busy();
```

The logs now showed the proper wait sequence:

```
I (xxxxx) mainichikatarenshu::ssd1680::interface: EPD_READBUSY: Waiting for busy pin to go LOW...
I (xxxxx) mainichikatarenshu::ssd1680::interface: Still waiting for BUSY pin... (count: 1M)
I (xxxxx) mainichikatarenshu::ssd1680::interface: Still waiting for BUSY pin... (count: 2M)
I (xxxxx) mainichikatarenshu::ssd1680::interface: EPD_READBUSY: BUSY pin is now LOW after 5654183 iterations
```

**Learning:** Every command was logged as sent and nothing reported an error, yet the screen stayed blank. Timing bugs on slow hardware produce no errors, so log the wait itself (the BUSY iteration count above) to prove that the code really waits.

**SD Prompt:** `Digital oscilloscope showing BUSY pin signal transitioning from HIGH to LOW, electronic measurement equipment, laboratory setting, technical precision, blue and green waveforms, professional electronics testing`

## Chapter 5: The Inverted World (October 14, 2025 - Afternoon)

### Black is White, White is Black

Success! The display was updating... but showing **all black** instead of white. The BUSY pin fix worked, but something was still wrong.

### Hardware Polarity Inversion

The display has inverted polarity compared to typical expectations:

- **Expected:** `0xFF` = white, `0x00` = black
- **This display:** `0xFF` = black, `0x00` = white ❌

This is a hardware/configuration difference in how the SSD1680 controller interprets RAM data, likely due to:

1. **Hardware wiring** - how the display panel is connected
2. **Controller configuration** - OTP (One-Time Programmable) settings in the SSD1680
3. **LUT (Look-Up Table)** - waveform settings controlling voltage application

### The Three-Part Fix

#### 1. Initialization Clear Value

```rust
// Before: self.interface.data_x_times(0xFF, total_bytes)?; // All white
// After:
self.interface.data_x_times(0x00, total_bytes)?; // All white (inverted polarity)
```

#### 2. Buffer Fill Value

```rust
// Before: let white_buffer = vec![0xFF; 64];
// After:
let white_buffer = vec![0x00; 64]; // All WHITE (inverted polarity)
```

#### 3. Remove Data Inversion

```rust
// The C++ inversion was compensating for different hardware
// Our display needs data sent directly
self.interface.cmd_with_data(Cmd::WRITE_BW_DATA, buffer)?;
```

### Understanding the Quirk

For this specific CrowPanel display:

- `0x00` byte = white pixels ⚪
- `0xFF` byte = black pixels ⚫

This affects `embedded-graphics` integration:

- `BinaryColor::Off` (0) → **white** on this display
- `BinaryColor::On` (1) → **black** on this display

**Learning:** The first fix (BUSY) revealed the next problem (polarity); an all-black screen is progress compared with a blank one. The inversion was applied in three separate places (clear value, buffer fill, send path), which is easy to get half-right, so state the colour mapping once where it matters (`BinaryColor::Off` = white) and keep it documented.

**SD Prompt:** `Yin-yang symbol made of e-paper display pixels, inverted colors concept, black and white contrast, philosophical technology fusion, minimalist art style, high contrast monochrome`

## Chapter 6: Code Archaeology (November 8, 2025 - Morning)

### Function Usage Analysis

With the driver working, I documented which functions were actually being used in production versus those created during development. Out of 30+ functions in the driver, only 7 were actively used in `main.rs`:

**Used Functions:**

1. `Ssd1680::new()` - Driver initialization
2. `cpp_init()` - Arduino-compatible initialization
3. `cpp_all_fill()` - Fill RAM with pattern
4. `cpp_update()` - Trigger display refresh
5. `cpp_clear_r26h()` - Clear RED RAM
6. `direct_cmd()` - Direct command access
7. `direct_data()` - Direct data access

**Unused but Valuable:**

- Test pattern functions (`draw_test_pattern()`, `white_and_black_test_pattern()`)
- Alternative update modes (`fast_update()`, `arduino_full_update()`)
- Power management (`sleep()`, `wake_up()`)
- Emergency recovery functions (`emergency_clear()`, `factory_reset_clear()`)

These unused functions represent the exploration process—different approaches tried, debugging tools created, and future capabilities planned. They're documented as `**UNUSED**` but kept for debugging and future power management features.

**Learning:** Only 7 of 30+ driver functions were used. Keeping the rest "for debugging" felt safe, but in November 2026 they were removed together with other uncalled code (Chapter 11), so unused code is only worth keeping while someone is actually likely to call it.

**SD Prompt:** `Archaeological dig site metaphor with layers of code, git history visualization, code evolution timeline, developer tools and artifacts, educational infographic style, clean vector graphics`

## Chapter 7: The Refactoring (November 8, 2025 - Late Morning)

### Reducing Repetition

With a working driver, it was time to clean up. The `driver.rs` file had grown to 1,744 lines with significant code duplication. Common patterns appeared 10+ times:

```rust
// Pattern: Reset with delay (12 occurrences)
self.interface.reset()?;
self.interface.delay.delay_ms(delay_ms);

// Pattern: Reset RAM counters (10 occurrences)
self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
self.interface.data(&[0x00])?;
self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
self.interface.data(&[0x00, 0x00])?;

// Pattern: Trigger display update (10 occurrences)
self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
self.interface.data(&[ctrl2_value])?;
self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
self.interface.wait_busy_low();
```

### Six Helper Functions

Extracted common patterns into reusable helpers:

1. **`reset_with_delay(delay_ms)`** - Hardware reset with delay
2. **`reset_ram_counters()`** - Set RAM X/Y counters to origin
3. **`reset_ram_counters_with_delay(delay_ms)`** - Combined version
4. **`trigger_display_update(ctrl2_value)`** - Update with BUSY wait
5. **`trigger_display_update_with_delay(ctrl2_value, delay_ms)`** - Update with pre-delay
6. **`set_full_ram_window()`** - Set RAM window to full frame

### Impact

- **Lines reduced:** 141 lines (1,744 → 1,603)
- **Functions refactored:** 11 major functions
- **Maintainability:** High - changes centralized
- **Readability:** Function names serve as inline documentation

Similar refactoring in `interface.rs` added 5 helper functions for pin operations and command sequences, reducing another 20 lines of repetition.

**Learning:** Six helpers removed only 141 of 1,744 lines; `driver.rs` stayed above 1,600 lines. Deduplicating inside a file is not the same as deciding what belongs in the file at all, and the larger reduction only came with the crate extraction and dead-code removal in Chapter 11.

**SD Prompt:** `Code refactoring visualization, before and after comparison, tangled spaghetti code transforming into clean organized modules, abstract technical illustration, blue and green color scheme, modern software engineering concept art`

## Chapter 8: The Great Refactoring (March 7, 2026)

### Beyond Code Cleanup

While Chapter 7 focused on reducing repetition within the driver, a more comprehensive refactoring was needed to address broader code quality concerns. The main application code in `main.rs` had grown with repetitive text rendering patterns, making maintenance difficult and testing nearly impossible.

### The Universal Text Problem

The application needed to display various types of text:

- WiFi status messages
- Date and time information
- Kata reminders
- Menu options
- Error messages

Each required similar but slightly different code patterns:

```rust
// ❌ REPEATED PATTERN - Manual text rendering
let text_style = MonoTextStyleBuilder::new()
    .font(&FONT_5X8)
    .text_color(BinaryColor::On)
    .build();

Text::new("WiFi Status", Point::new(200, 10), text_style)
    .draw(&mut display).ok();

// ❌ REPEATED PATTERN - Manual multiline handling
let lines = message.split('\n');
for (i, line) in lines.enumerate() {
    let y = 10 + (i * 20); // Manual line spacing
    Text::new(line, Point::new(10, y), text_style)
        .draw(&mut display)?;
}
```

### The Solution: Modular Architecture

Created four new modules to address different concerns:

#### 1. Universal Text Rendering (`src/ssd1680/text.rs`)

**Key Innovation:** `TextRenderer::write_text()` - A single function that handles text of any length with automatic multiline support.

```rust
// ✅ UNIVERSAL SOLUTION - One function for all text
TextRenderer::write_text(
    display,
    "Line 1\nLine 2\nVery long line that wraps automatically",
    10,  // x position
    50,  // y position
    0,   // auto-detect width
    TextConfig::default()
        .alignment(TextAlignment::Center)
        .color(BinaryColor::On)
)?;
```

**Features:**

- Automatic text wrapping based on display width
- Multiple alignment options (left, center, right)
- Configurable fonts, colors, and line spacing
- Text measurement utilities
- Single-line and multiline support

#### 2. Display Utilities (`src/ssd1680/display_utils.rs`)

Extracted common display patterns into reusable functions:

```rust
// ✅ HIGH-LEVEL UTILITIES - Common patterns
DisplayManager::create_display();                    // Standard initialization
DisplayManager::clear_and_prepare(display);           // Clear with proper settings
DisplayManager::show_status_message(display, "WiFi Connected", None);
DisplayManager::show_datetime_status(display, "2025-03-07", "18:00", Some("WiFi OK"));
```

**Predefined configurations** for common use cases:

- `presets::title()` - Large font for titles
- `presets::body()` - Standard body text
- `presets::status()` - Right-aligned status messages
- `presets::centered()` - Centered text
- `presets::large_centered()` - Prominent centered text

#### 3. Application-Specific Functions (`src/kata_display.rs`)

Separated application logic from generic utilities:

```rust
// ✅ APPLICATION-SPECIFIC - Kata reminder workflow
KataDisplayManager::show_kata_reminder(
    display,
    "2025-03-07",
    "18:00",
    "Heian Shodan",
    Some("WiFi Connected")
)?;

KataDisplayManager::show_completion_screen(
    display,
    "Heian Shodan",
    "18:05"
)?;
```

**Features:**

- Complete kata reminder screens
- Training statistics display with `TrainingStats` struct
- Menu system with selection highlighting
- Error and informational messages
- Motivational messages

#### 4. Comprehensive Testing (`src/display_tests.rs`)

Created extensive test suite covering all new functionality:

```rust
#[test]
fn test_universal_text_writing() {
    let mut display = MockDisplay::new();
    let result = TextRenderer::write_line(
        &mut display,
        "Hello World",
        10, 20,
        TextConfig::default(),
    );
    assert!(result.is_ok());
}

#[test]
fn test_multiline_text_handling() {
    let multiline_text = "Line 1\nLine 2\nLine 3";
    let height = TextRenderer::write_text(
        &mut display,
        multiline_text,
        10, 20, 0,
        TextConfig::default(),
    );
    assert!(height.is_ok());
    assert!(height.unwrap() > 0);
}
```

### Impact on Main Application

The `main.rs` file transformed from repetitive manual text handling to clean, declarative calls:

**Before:**

```rust
// ❌ MANUAL - 20+ lines of repetitive code
let mut display = Display2in13::new();
display.set_rotation(DisplayRotation::Rotate270);
display.clear(BinaryColor::Off)?;

let text_style = MonoTextStyleBuilder::new()
    .font(&FONT_5X8)
    .text_color(BinaryColor::On)
    .build();

let wifi_text = Text::new(&wifi_status, Point::new(200, 10), text_style);
wifi_text.draw(&mut display).ok();

let date_text = get_rtc_date();
let text_style = MonoTextStyleBuilder::new()
    .font(&ISO15_10)
    .text_color(BinaryColor::On)
    .build();

Text::new(&date_text, Point::new(10, 30), text_style)
    .draw(&mut display)?;

let label_style = MonoTextStyle::new(&FONT_5X8, BinaryColor::On);
Text::new("Current Date:", Point::new(10, 10), label_style)
    .draw(&mut display)?;
```

**After:**

```rust
// ✅ DECLARATIVE - 5 lines of clear intent
let mut display = DisplayManager::create_display();
DisplayManager::clear_and_prepare(&mut display)?;

let date_text = get_rtc_date();
let time_text = "18:00".to_string();
let kata_name = "Heian Shodan";

KataDisplayManager::show_kata_reminder(
    &mut display,
    &date_text,
    &time_text,
    kata_name,
    Some(&wifi_status),
)?;
```

### Quantified Improvements

**Code Reduction:**

- **main.rs:** Reduced from ~50 lines of display code to ~10 lines
- **Text rendering:** 1 universal function replaces 8+ manual patterns
- **Display patterns:** 6 utility functions replace 15+ repetitive code blocks

**Maintainability:**

- **Single source of truth** for text rendering logic
- **Centralized configuration** in presets
- **Clear separation** between generic utilities and application logic
- **Consistent API** across all display operations

**Testability:**

- **Pure functions** that accept generic display traits
- **Mock-friendly** design for unit testing
- **Comprehensive coverage** with 15+ test functions
- **Integration tests** for complete display pipelines

**Reusability:**

- **Universal text function** handles any text length automatically
- **Configurable styling** through `TextConfig` builder pattern
- **Generic design** works with any display implementing `Display` trait
- **Extensible architecture** for new display features

### The Builder Pattern

Implemented fluent builder pattern for text configuration:

```rust
let config = TextConfig::new(&FONT_10X20)
    .color(BinaryColor::On)
    .alignment(TextAlignment::Center)
    .line_spacing(2);
```

This makes configuration intuitive and discoverable while providing compile-time safety.

### Training Statistics System

Created comprehensive `TrainingStats` struct with full lifecycle management:

```rust
let mut stats = TrainingStats::new(10, 3, 5, 7);
stats.session_completed();        // Updates all relevant fields
stats.reset_weekly();           // Weekly maintenance
stats.break_streak();           // Handle missed days
```

**Learning:** This refactoring built generic structure ahead of its callers. Within eight months `TrainingStats`, five of the screens, `display_utils.rs`, `kata_display.rs`, `display_tests.rs` and `tests/unit_tests.rs` had been deleted or replaced (`c48e4d3`, Chapter 11). Abstraction is worth the cost when something already calls it; tests that only restate other tests add maintenance and no safety.

**SD Prompt:** `Modular software architecture visualization, showing separate modules for text rendering, display utilities, application logic, and testing, connected by clean interfaces, professional software design diagram, blue and white color scheme`

## Technical Achievements (March 2026 Snapshot)

_Historical note: the structure and roadmap below capture the project at the end of Chapter 8 (March 2026), before the bare-metal `esp-hal` migration, the `crowpanel-ssd1680` workspace crate extraction, and the November 2026 refactoring. See **Current Architecture and What's Next** near the end of this document for the present layout._

### Driver Architecture (March 2026)

The final driver structure:

```
src/ssd1680/
├── cmd.rs          # Command definitions (0x00-0xFF)
├── color.rs        # Color handling (BW, Red)
├── driver.rs       # Main driver (1,603 lines)
├── interface.rs    # SPI communication (279 lines)
├── graphics.rs     # Drawing primitives
├── pins.rs         # GPIO configuration
├── text.rs         # Universal text rendering utilities
├── display_utils.rs # Generic display operations
└── mod.rs          # Module exports

src/
├── kata_display.rs  # Application-specific display functions
└── main.rs         # Simplified main application

tests/
├── unit_tests.rs   # Unit tests with MockDisplay
└── state_storage.rs # Flash storage integration tests
```

### Key Features

- **Hardware-specific:** Tailored to CrowPanel ESP32 E-paper display
- **Arduino-compatible:** Functions matching working C++ implementation
- **Embedded-graphics ready:** Integration with Rust graphics ecosystem
- **Universal text rendering:** Single function handles any text length with automatic multiline
- **Modular architecture:** Clear separation of concerns across 4 new modules
- **Comprehensive testing:** 15+ test functions with full coverage
- **Maintainable design:** Centralized configuration and reusable utilities
- **Power-efficient:** Deep sleep support (planned)
- **Well-documented:** Every quirk and fix documented inline

### Performance Characteristics

- **Full update:** ~2 seconds (BUSY pin wait)
- **Fast update:** ~1 second (more ghosting)
- **SPI speed:** 40MHz (ESP32-S3 default)
- **Memory usage:** Minimal (streaming updates)

**SD Prompt:** `Technical architecture diagram showing ESP32-S3 chip connected to SSD1680 e-paper display controller, SPI bus visualization, GPIO pins labeled, professional electronics schematic style, clean lines, blue and black color scheme`

## Lessons Learned

### 1. Trust the Working Code

When datasheets conflict with working implementations, **trust the working code**. The `0xF4` vs `0xC7` difference was crucial—the datasheet was technically correct but not optimal for this hardware configuration.

### 2. Hardware Has Quirks

The polarity inversion wasn't a bug—it was a hardware characteristic. Different displays with the same controller can behave differently based on wiring, OTP settings, and LUT configuration.

### 3. E-Paper is Slow

Always wait for the BUSY pin. E-paper displays are fundamentally different from LCDs—they're slow, physical, and require patience. Rushing leads to blank screens and corrupted states.

### 4. Document the Journey

The five markdown files created during development became invaluable:

- `DRIVER_UPDATES.md` - What changed and why
- `CRITICAL_FIX.md` - The BUSY pin revelation
- `POLARITY_FIX.md` - Understanding inverted polarity
- `FUNCTION_USAGE.md` - Code archaeology
- `REFACTORING_SUMMARY.md` - Cleanup process

### 5. Keep Debug Code (With Limits)

Functions marked `**UNUSED**` aren't waste while the hardware is still being understood—test patterns, emergency clears, and alternative update modes proved invaluable during development. Once the driver was stable and extracted into its own crate, uncalled code was removed (Chapter 11); keep exploration tools only for as long as they are plausibly needed.

### 6. Invest in Abstraction

The universal text function demonstrates the power of good abstraction. One function handling any text length with automatic multiline eliminated 8+ repetitive patterns and made the codebase dramatically more maintainable.

### 7. Separate Concerns Early

Breaking code into logical modules (text rendering, display utilities, application logic, testing) from the start prevents technical debt. Each module has a single responsibility and clear interfaces.

### 8. Testability is Design

Designing for testability isn't an afterthought—it's fundamental. Pure functions that accept generic traits make comprehensive testing possible and catch bugs early.

### 9. Change One Thing, With a Hypothesis

The January 2025 history is a run of commits that reshuffled code without learning anything ("that did not help", "still crash when running"). Progress came from logging the BUSY wait, then from a line-by-line comparison with the working Arduino code.

### 10. Pin What Must Work Together

`esp-idf-svc`, `epd-waveshare` and the later `esp-radio` all needed specific versions to compile together. Pin them, exclude them from automatic updates until verified, and record why.

### 11. Delete in the Same Change

Dead modules left behind by a migration (`src/input/`, `src/wifi.rs`, `kukkuu/`) and abstractions built before their callers (`display_utils`, `TrainingStats`) were all removed later. Removing code while the reason is still fresh is cheaper.

**SD Prompt:** `Open notebook with handwritten technical notes, sketches of circuit diagrams, coffee cup, mechanical keyboard, developer's desk from above, warm lighting, productive workspace aesthetic, photorealistic`

## Resources and References

### Hardware

- [CrowPanel ESP32 2.9" E-paper Display](https://www.elecrow.com/crowpanel-esp32-2-9-e-paper-hmi-display-with-128-296-resolution-black-white-color-driven-by-spi-interface.html)
- [SSD1680 Datasheet](https://www.good-display.com/companyfile/32.html)
- [ESP32-S3 Technical Reference Manual](https://www.espressif.com/sites/default/files/documentation/esp32-s3_technical_reference_manual_en.pdf)

### Software

- [ESP-RS Book](https://esp-rs.github.io/book/) - Rust on ESP32
- [embedded-graphics](https://github.com/embedded-graphics/embedded-graphics) - 2D graphics library
- [esp-idf-hal](https://github.com/esp-rs/esp-idf-hal) - Hardware abstraction layer

### Inspiration

- [Elecrow Arduino Examples](https://github.com/Elecrow-RD/CrowPanel-ESP32-E-Paper) - Working C++ implementation
- [mbv/ssd1680](https://github.com/mbv/ssd1680) - Initial Rust driver structure
- [epd-waveshare](https://github.com/caemor/epd-waveshare) - Alternative E-paper drivers

**SD Prompt:** `Stack of technical books and datasheets, ESP32 development board, e-paper display, electronic components scattered on workbench, maker space atmosphere, warm workshop lighting, detailed and realistic`

## What's Next (March 2026 Snapshot)

_Historical note: most of these goals were later reshaped and implemented in Chapters 9–12 (for example, local `kata.yaml` embedding instead of fetching katas online, and Supabase HTTPS sync instead of Google Drive)._

### Immediate Goals

1. **WiFi Integration** - Fetch kata list from online source
2. **RTC Setup** - Wake at 7:00 AM, sleep at 11:00 PM
3. **Button Handling** - Confirm kata completion
4. **Google Drive Sync** - Track training history

### Future Enhancements

1. **Electricity Price Display** - Show current energy costs
2. **Weather Integration** - Display conditions for outdoor training
3. **Battery Optimization** - Deep sleep between updates
4. **OTA Updates** - Wireless firmware updates

### Code Quality

1. **Unit Tests** - Test helper functions independently
2. **Integration Tests** - Verify display sequences
3. **Documentation** - Rustdoc for public API
4. **Examples** - Simple usage patterns

**SD Prompt:** `Futuristic e-paper display showing Japanese kanji, weather icons, and graphs, mounted on wall, smart home integration, minimalist interior design, soft ambient lighting, technology seamlessly integrated into daily life`

## Chapter 9: Breaking Free from ESP-IDF (November 2026)

### Why Leave ESP-IDF?

While ESP-IDF provided a rapid initial path to prototype hardware peripherals using standard library abstractions, it carried significant friction:

- **Heavyweight C toolchain dependencies:** Compiling required CMake, Ninja, Python virtual environments, and C bindings via `esp-idf-sys`.
- **Scaffolding overhead:** Generating custom bootloaders required maintaining a parallel C boilerplate project (`kukkuu`) and managing `sdkconfig` files.
- **Binary footprint and memory usage:** Linking the full FreeRTOS and ESP-IDF C runtime bloated the binary and complicated memory budgeting.
- **Desire for pure Rust (`no_std`) reliability:** Moving to bare-metal Rust provides fine-grained control over allocations, strict compile-time safety, faster incremental build times, and zero mystery runtime tasks.

### The Bare-Metal Target Shift

The project moved target architectures from `xtensa-esp32s3-espidf` (with standard library support) to bare-metal `xtensa-esp32s3-none-elf` with `build-std = ["core", "alloc"]`.

```toml
# .cargo/config.toml
[build]
target = "xtensa-esp32s3-none-elf"

[target.xtensa-esp32s3-none-elf]
runner = "espflash flash --monitor" # later: + --partition-table partitions.csv
rustflags = ["-C", "link-arg=-Tlinkall.x"]

[unstable]
build-std = ["core", "alloc"]
```

### The `esp-hal` & Embassy Architecture

Replacing ESP-IDF meant transitioning to modern bare-metal Espressif crates:

- **`esp-hal` (v1.2.0):** Direct hardware registers and peripherals (GPIO, SPI, UART, RTC, Timers) without FreeRTOS overhead.
- **`esp-alloc`:** Lightweight in-RAM heap allocator for dynamic data structures (`heapless` and `alloc`).
- **`esp-println`:** Immediate logging via UART or critical section without heavy logging daemons.
- **`esp-radio` & `embassy-net`:** Station mode Wi-Fi powered by Espressif's radio driver and Embassy async network stack.

The main entry point was rewritten using `#[esp_hal::main]` (or `#[esp_rtos::main]` when Wi-Fi is enabled) with explicit peripheral ownership and async task coordination.

### Retiring the Bootloader Project

With `espflash` managing partition tables and flashing directly, the helper C project `kukkuu/` and `sdkconfig.defaults` were retired and removed.

The bootloader that `espflash` supplies is still an ESP-IDF 2nd stage bootloader, and it refuses an `esp-hal` image that lacks the ESP-IDF application descriptor (`ESP-IDF App Descriptor ... missing in your esp-hal application`). The fix is the `esp-bootloader-esp-idf` crate and one `esp_bootloader_esp_idf::esp_app_desc!();` line in `src/main.rs`, which adds a `.flash.appdesc` section to the ELF. Both release builds linked fine without it, so only flashing revealed the gap (Chapter 1's lesson again: the build passing says nothing about what the bootloader accepts).

The first run on the CrowPanel then booted the app and panicked with `Invalid or missing training partition: MissingTraining`. `espflash flash` writes its own default partition table (`nvs`, `phy_init`, `factory`) at `0x8000` unless `--partition-table` is given, so the `training` partition that `partitions.csv` defines never existed on the device. The firmware's partition validation did its job; the README just had to put the partition table step before the first flash, and the runner in `.cargo/config.toml` now passes `--partition-table partitions.csv` so every `cargo run` installs it.

The next run reached the display and panicked with `Display update failed: BusWriteError`. The ESP-IDF-era code had enabled the panel supply on GPIO7 (`LCD_3.3_CTL`, as the Arduino example does) and waited 100 ms; the rewrite only kept the pin number as a constant in `pins.rs` and never drove it. `main.rs` now sets GPIO7 high and waits 100 ms before touching the panel. Unpowered, the controller cannot answer, and `BusWriteError` is also what the driver reports for a BUSY timeout, so the error name pointed at SPI while the cause was power. A migration has to carry over side effects such as enabling a supply pin, not only the API calls. Tooling configurations (`wokwi.toml`, VS Code launch profiles) were updated to target the `xtensa-esp32s3-none-elf` binary.

**Learning:** The migration commit (`de6fedf`, "Almost full refactor to drop out ESP-IDF") left the ESP-IDF-era modules (`src/input/`, `src/wifi.rs`, `src/app.rs`) and the `kukkuu/` project in the tree for two weeks until `c48e4d3` removed them. Delete the old stack in the same change that replaces it, so nobody has to guess which code is live. Also, when a project moves from a framework to bare metal, the bootloader and partition table become the project's own responsibility (Chapter 10).

**SD Prompt:** `A spacecraft jettisoning heavy booster stages to reveal a sleek, lightweight satellite, minimalism concept, cybernetic aesthetic, clean blue and silver lines, high tech digital art`

## Chapter 10: Robust Flash Persistence & Monotonic Input (November 2026)

### Partition Validation in Rust (`src/partitions.rs`)

Rather than trusting undocumented flash offsets or relying on ESP-IDF NVS wrappers, the firmware now inspects and parses the ESP32 partition table directly from raw flash:

- Reads the partition table from offset `0x8000`.
- Validates the table structure and verifies the MD5 checksum.
- Asserts that a dedicated `training` partition (128 KiB at `0x420000`) exists, is correctly sector-aligned, and does not overlap with application or NVS regions.
- Fails fast before mounting any state storage.

### Atomic State Storage (`src/storage.rs`)

Training state snapshots are managed through `sequential-storage` using `postcard` serialization over flash sectors:

- Power-loss safe snapshots preserve the active day's kata and completion timestamp.
- Queue holds up to 32 pending records for later synchronization.
- Backwards time jumps are detected and rejected to protect training streak integrity.

### Monotonic Button Debouncer (`src/buttons.rs`)

The old button handling relied on FreeRTOS task notifications and OS threads (`src/input/`). The new architecture introduces a pure monotonic clock debouncer:

- Tracks millisecond timestamps from the RTC timer.
- Detects transitions without background polling tasks or thread allocations.
- Rejects monotonic clock inconsistencies while remaining completely `no_std` compatible.

**Learning:** The earlier bootloader and partition-table confusion (Chapter 1) became a design rule: validate the real flash layout at startup and refuse to run on a mismatch, rather than assuming the table that was intended is the table that is installed. The queue is also still unproven against real power loss, which is why the README calls it experimental.

## Chapter 11: Thin Firmware, Testable Library (November 2026)

After a burst of feature work (SNTP, saved Wi-Fi credentials, spot prices, Supabase sync, five buttons, rest pauses), `src/main.rs` had grown into a single ~470-line `application()` function. It mixed peripheral setup, UART parsing, channel polling, button handling, kata selection, flash transactions, screen choice and sleep. None of that could be tested on a host machine. A review of the whole codebase found improvements, but also bugs hiding in the noise.

### Bugs Found by Reading, Not Running

- **The `wifi` build did not compile.** `embedded-io-async` was used in `wireless.rs` but never declared in `Cargo.toml`, so CI had been failing on the Wi-Fi build.
- **Shadowed state.** `let mut wifi_submitted = false;` appeared twice in `main.rs`. The second declaration reset the flag after auto-connecting with saved credentials, so a later `CONNECT` sent credentials the Wi-Fi task would never read.
- **Invisible text.** Right-aligned text added the full display width to `x`, so the Wi-Fi status and the price were drawn past the edge of the 128-pixel panel. The existing tests even had comments saying so.
- **Prices off by ten.** Querying the live porssisahko.net API showed it returns **c/kWh**, while the code treated the value as EUR/MWh. The code also took the newest published hour instead of the current one, rounded negative prices incorrectly, and expected a full price list that didn't fit its 1 KiB receive buffer.
- **Duplicate uploads.** The main loop re-queued the oldest pending record as soon as the network task picked it up, so the same day could be posted to Supabase twice.
- **Silent truncation.** Supabase requests were written into a 512-byte `heapless::String` and overflow was ignored. A real anon key (a JWT, sent twice in the headers) would have produced a cut-off request.

### Extracting Decisions From Hardware

The SSD1680 chipset driver first moved into its own workspace crate (`crates/crowpanel-ssd1680`), separating panel command sequences and frame buffers from application rules. Then the application itself was split along one guiding rule: **if it decides something, it belongs in the library; if it touches a peripheral, it stays in the binary.**

| New / reshaped module   | What moved there                                                             |
| ----------------------- | ---------------------------------------------------------------------------- |
| `src/clock.rs`          | RTC timestamp validation; replaces three copies of the magic `946_684_800`   |
| `src/console.rs`        | `LineBuffer` (byte accumulation, overflow, non-ASCII) and a `Command` parser |
| `src/screens.rs`        | Pure `select_screen()` returning a `Screen` enum, plus `draw_screen()`       |
| `src/layout.rs`         | Former `DisplayManager` unit struct, now free `draw_*` functions             |
| `src/http.rs`           | HTTP status parsing shared by the price and Supabase clients                 |
| `training::choose_kata` | Unbiased rejection sampling over the kata list                               |
| `ButtonBank::pressed`   | Samples all buttons at once and returns new presses                          |

`main.rs` is now a set of `init_*` functions plus an `App` struct with one small method per concern: `poll_console`, `poll_buttons`, `update_training`, `refresh_display`, `rest_pause`, and `poll_network` for `wifi` builds. Four copies of the "panic on ambiguous flash write" match collapsed into a single `check_storage` helper.

In `wireless.rs`, three copies of DNS lookup, TCP connect and read-until-close became `resolve_host` and `http_exchange`. `wifi_task` was split into station configuration, network start and a per-connection `Session` loop. The session refreshes the price once per Helsinki-local hour and retries failed SNTP and price requests every minute.

### Generic Storage Without a Format Change

`storage.rs` had two hand-written snapshot structs and three copies of "clone state, apply, save, publish". Postcard does not encode field names, so a single generic type is byte-compatible with both old snapshots:

```rust
#[derive(Deserialize, Serialize)]
struct Versioned<T> {
    version: u8,
    value: T,
}
```

A `transact()` helper now carries the save-before-publish rule for `advance`, `confirm` and `acknowledge`, and records already stored in flash remain readable.

### Deleting What Nobody Called

Several items had their own tests but were never called by the firmware: a WPA2 key derivation module (with its `pbkdf2` and `sha1` dependencies), `TrainingStats`, five unused screens, area-clearing text helpers, and a build-time logo conversion whose output was never embedded. The logo step also printed about ten `cargo:warning` lines on every build. All of it was removed, along with `tests/unit_tests.rs`, which only repeated in-module tests.

### Configuration and Honesty

Supabase settings now come from `SUPABASE_HOST`, `SUPABASE_ANON_KEY` and `SUPABASE_TABLE` at build time, the same way `KATA_CONFIG` works, so no key is ever committed. At the time of that refactor, the README stated the remaining blocker plainly: both the price API and Supabase were **HTTPS-only**, and plain HTTP got a `301` redirect until a verified TLS client was added in Chapter 12.

### Learning From The Bugs

- A feature build that nobody built was broken for a long time. Build every feature combination in CI (`firmware` and `wifi` now both are).
- A duplicated `let mut` is invisible in a 470-line function. Small functions with one concern make shadowing and reset mistakes visible, and make them testable.
- Unit tests that mirror the author's assumptions pass while the assumption is wrong (the existing tests even carried comments about off-screen text). Compare against the real thing: the live API showed c/kWh, not EUR/MWh.
- Fixed-size buffers need an overflow decision, not an ignored `Result`.
- Host coverage moved from 37.99% to 94.46% in one commit (`143b6e5`) once decisions lived in the library; it later settled around 92% as TLS and HTTP code grew, and the 90% gate in `tarpaulin.toml` keeps that from sliding.

### Guard Rails

- `#![warn(missing_docs)]` on the library, with every public item documented.
- The `write_text` doctest, which had never compiled, was fixed and passes locally via `cargo +stable test --workspace --doc`.
- To conserve CI minutes, GitHub Actions builds both firmware release feature sets (`firmware` and `wifi`), checks formatting, runs firmware clippy on the release Wi-Fi build, and runs host unit/integration tests (`--lib --tests`) and host clippy with `-D warnings`.
- Documentation tests (`--doc`) and `cargo +stable tarpaulin` (enforcing the 90% coverage gate configured in `tarpaulin.toml`) are run locally. Firmware clippy runs on library/binary targets directly (`--features firmware` / `--features wifi`), while host clippy runs with `--target <host-triple> --features test-utils`. Running `cargo clippy --all-targets` without a host target fails because test harnesses require `std`, which does not exist on `xtensa-esp32s3-none-elf` (also why `.vscode/settings.json` disables `rust-analyzer.check.allTargets`).

**SD Prompt:** `Sculptor carving a sleek minimal statue out of a rough block of stone, chips falling away, workshop with neatly organized tools, warm light, black and white ink wash style`

## Chapter 12: Authenticated HTTPS, Quarter-Hour Prices, and Hardened Framing (November 2026)

With the application logic untangled, one functional blocker still stood between the firmware and live services: both `api.porssisahko.net` and Supabase reject plaintext HTTP. Closing that gap meant implementing real, authenticated HTTPS on bare-metal `xtensa-esp32s3-none-elf` without sacrificing host testability.

### Choosing a Verified `no_std` TLS Stack

Encrypting bytes is not the same as authenticating a server. On an embedded target without an operating system, three failure modes are easy to overlook: accepting untrusted chains, mishandling X.509 Subject Alternative Names (falling back to Common Name even when SANs exist), and skipping `notBefore` / `notAfter` certificate validity dates because no wall clock is wired up.

`mbedtls-rs` (`0.3`, backed by MbedTLS 3.x) fit the existing `esp-hal 1.2`, `embassy-time 0.5`, and `embedded-io-async 0.7` stack cleanly on both Xtensa and host targets:

- **Hardware entropy:** `TrngSource` (`RNG` + `ADC1`) feeds `Trng` for TLS 1.2 and TLS 1.3 handshakes.
- **Fail-closed wall-clock dates:** Enabling `hook-wall-clock` compiles `MBEDTLS_HAVE_TIME_DATE` into MbedTLS and routes X.509 time checks through `tls::WallClock` (fed by SNTP, manual `TIME <unix_secs>` over UART, or an RTC preserved across deep sleep). When no valid time is set, the hook returns `None` and certificate validation fails closed.
- **Curated, rotatable trust anchors:** Eight standard WebPKI root CAs covering Cloudflare, Google Trust Services, GlobalSign, Let's Encrypt (`ISRG Root X1`/`X2`), DigiCert, and SSL.com live in `certs/ca-roots.pem`, validated at build time and overridable via `TLS_CA_BUNDLE` without code changes.
- **Host-side TLS fixture verification:** Using an in-memory async duplex pipe, the exact `tls::https_exchange` client is tested on the host against local X.509 fixtures for valid `Content-Length` and `chunked` exchanges, wrong hostname, RFC 6125 SAN-only enforcement (rejecting a matching `CN` when `DNS` SANs do not match), untrusted CA, expired certificate, not-yet-valid certificate, and missing clock.

### Quarter-Hour Spot Pricing (`porssisahko.net` v2) and Hardened HTTP

Inspecting the live `porssisahko.net` documentation confirmed that Finland's spot market now runs on 15-minute (_varttisähkö_) intervals under **API v2** (`GET /v2/price.json?date=YYYY-MM-DDTHH:MM:SS.000Z`). Switching from `/v1/price.json` to `/v2/price.json` reports the actual current quarter-hour price in `c/kWh` (VAT included) and queries in UTC (`Z`), eliminating the repeated local-hour ambiguity during the autumn DST fallback.

At the protocol boundary:

- `src/http.rs` validates status codes and headers, bounds buffer capacity (2 KiB for Cloudflare header headroom), decodes `Transfer-Encoding: chunked` in place, stops immediately when self-delimited responses (`Content-Length`, `chunked`, `204`/`304`) complete, and rejects ambiguous (`Transfer-Encoding` + `Content-Length`), truncated, or oversized responses.
- `src/electricity.rs` parses `{"price": ...}` with `serde_json` rather than scanning raw response text, preserving integer tenths-of-a-cent rounding, and `PriceUpdates` only marks a 15-minute slot complete once the display channel accepts the update.
- `src/supabase.rs` extracts `RecordSync` and `OutcomeDelivery` so uploads wait for a valid clock, back off for 60 seconds after failures instead of retrying every poll cycle, and retain `SyncOutcome` across channel saturation rather than dropping an acknowledgement or wedging `in_flight`.

We also evaluated extracting `src/electricity.rs` into a second workspace crate alongside `crates/crowpanel-ssd1680` and chose to defer: unlike the display driver, the price module is compact (~130 lines of logic), shares this firmware's display formatting and 15-minute refresh state, and has no second consumer yet.

### Learning From The Plan Changing

- An API can change under the code. The v1 hourly price endpoint became v2 quarter-hour prices, and a plan that had assumed plain HTTP met servers that only answer on HTTPS. Read the service's current documentation before building a client.
- Encryption is not authentication. Chain, hostname (SAN-only), validity dates and a trustworthy clock each need their own test, and the clock itself comes from unauthenticated SNTP, which is why that limitation is stated in the README rather than hidden.
- Keep unfinished claims honest: hardware verification is still pending, so documentation says "experimental" instead of "working".

## Current Architecture and What's Next

### Current Workspace Layout

```text
crates/crowpanel-ssd1680/   # Standalone no_std SSD1680 e-paper driver crate (18 unit tests)
certs/ca-roots.pem          # Default WebPKI root CAs for HTTPS (overridable via TLS_CA_BUNDLE)
build.rs, build_support/    # Compile-time validation for kata.yaml, Supabase config, and CA PEMs
src/
├── main.rs                 # Thin ESP32-S3 peripheral setup and polling loop (`firmware` bin)
├── wireless.rs             # Station Wi-Fi, SNTP, and port-443 HTTPS tasks (`wifi` feature)
├── tls.rs                  # Authenticated MbedTLS 1.2/1.3 client and fail-closed WallClock
├── http.rs                 # Bounded HTTP/1.x parser with Content-Length and chunked decoding
├── electricity.rs          # porssisahko.net v2 15-minute price paths, JSON parsing, slot state
├── supabase.rs             # PostgREST request formatting and at-least-once sync coordination
├── sntp.rs, clock.rs       # SNTP packet handling and >= 2000-01-01 UTC validation
├── training.rs, schedule.rs# Daily kata state machine and Helsinki 07:00–23:00 DST schedule
├── storage.rs, partitions.rs # MD5-checked partition lookup and sequential-storage flash queue
├── buttons.rs, console.rs  # Monotonic 5-button debouncer and ASCII UART command parser
└── screens.rs, layout.rs, text.rs # Pure screen selection and embedded-graphics layout
tests/
├── kata_config_tests.rs    # Validates kata.yaml and certs/ca-roots.pem parser rules
└── state_storage.rs        # Simulated flash persistence and interrupted-write recovery tests
```

Current verified baseline:

- **94 automated host tests** (`18` driver + `64` library + `12` integration) plus workspace doctests, with **`91.92%` (`1035/1126`)** line coverage measured by `cargo +stable tarpaulin`.
- **Linked ESP32-S3 release builds:** `~170 KiB` loadable flash (`firmware`) and `~825 KiB` loadable flash (`wifi`, including Wi-Fi, Embassy, MbedTLS 3.x, and root CAs) inside the 4 MiB `factory` partition.

### Prioritized Next Steps

1. **On-device verification:** Exercise Wi-Fi association, SNTP, live TLS handshakes against `api.porssisahko.net` and a test Supabase table, flash power-loss behavior, and current draw on a physical CrowPanel ESP32-S3.
2. **True deep sleep and GPIO wake:** Replace bounded polling pauses during the 23:00–07:00 rest window with ESP32-S3 deep sleep woken by the `07:00` Helsinki RTC alarm or the `CONF` button (`GPIO5`).
3. **Remaining button actions:** Assign concrete workflows (such as manual refresh, history inspection, or status screens) to `MENU`, `EXIT`, `UP`, and `DOWN`.
4. **Time and trust hardening:** Address SNTP's lack of cryptographic authentication (for example via NTS or monotonic lower-bound persistence in flash) and evaluate flash-stored CA bundle updates without reflashing.

## Conclusion

Building a Rust driver for SSD1680 E-paper display was a journey through hardware quirks, toolchain battles, and careful code archaeology. The key was patience—both in waiting for the BUSY pin and in methodically comparing working implementations.

The result is a robust, well-documented driver that handles the specific characteristics of the CrowPanel ESP32 E-paper display. Every bug became a learning opportunity, every fix was documented, and unused code remains as a testament to the exploration process.

The March 2026 refactoring transformed the codebase from functional but repetitive to clean, maintainable, and thoroughly tested. The universal text rendering function alone eliminated 8+ repetitive patterns, while the modular architecture ensures future development remains organized and testable.

Migrating from the ESP-IDF framework to bare-metal `esp-hal` delivered a lightweight, fast-building, and robust foundation with pure Rust partition safety, atomic flash snapshots, and async Embassy networking.

The November 2026 cleanup applied the same lesson again: once feature work piles up in the entry point, bugs hide in plain sight. Pulling every decision into small, host-tested library functions left `main.rs` as thin hardware glue, surfaced six real bugs along the way, and made adding authenticated MbedTLS HTTPS and quarter-hour spot pricing verifiable on the host before touching hardware.

The project continues toward its goal: a daily kata reminder that combines traditional martial arts practice with modern embedded systems, all written in clean, maintainable Rust.

**毎日型練習して！** (Train kata every day!)

**SD Prompt:** `Zen garden with modern e-paper display showing Japanese calligraphy, fusion of traditional and modern, peaceful atmosphere, black and white aesthetic, minimalist composition, inspirational technology meets tradition concept`

## Appendix: Raw Notes from the ESP-IDF Period

_Moved from the README. These are the original working notes from January–February 2025 and October 2025, kept verbatim as a record. The ESP-IDF steps, the custom `--bootloader` runner and the `kukkuu` project are no longer used: the current runner in `.cargo/config.toml` is `espflash flash --monitor --partition-table partitions.csv`, which supplies its own bootloader. Chapters 1 and 2 summarize them and list what was learned._

### Boot loader (ESP-IDF period)

https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/bootloader.html

Initial bootloader was:

```
ESP-IDF v5.1-beta1-378-gea5e0ff298-dirt 2nd stage bootloader
```

Started by installing for Windows
https://dl.espressif.com/dl/esp-idf/

Choosing the latest version (5.4. at the time, released just two days ago) and installing it in `C:\Espressif\frameworks\esp-idf-v5.4`.

```sh
idf.py create-project kukkuu
cd kukkuu
```

Then using "menuconfig" to create some defaults:

```sh
idf.py menuconfig
```

Editing `sdkconfig` to `CONFIG_IDF_TARGET="esp32s3"` and `CONFIG_ESPTOOLPY_FLASHSIZE_8MB=y`.

Then needed to clean first

```sh
idf.py fullclean
```

Finally creating the boot loader:

```sh
idf.py bootloader
```

How does the bootloader image look like?

```sh
esptool.py image_info .\build\bootloader\bootloader.bin
```

```
esptool.py v4.8.1
File size: 22352 (bytes)
Detected image type: ESP32-S3
Image version: 1
Entry point: 403c899c
4 segments

Segment 1: len 0x0164c load 0x3fce2810 file_offs 0x00000018 [BYTE_ACCESSIBLE,MEM_INTERNAL,DRAM]
Segment 2: len 0x00004 load 0x403c8700 file_offs 0x0000166c [MEM_INTERNAL,IRAM]
Segment 3: len 0x00d7c load 0x403c8704 file_offs 0x00001678 [MEM_INTERNAL,IRAM]
Segment 4: len 0x03320 load 0x403cb700 file_offs 0x000023fc [MEM_INTERNAL,IRAM]
Checksum: 96 (valid)
Validation Hash: 2527a34d14de8e76959bd7d8c7429d897cc37831c2c6774515a4074df043c74f (valid)
```

Flashing is easy via same tool:

```sh
idf.py bootloader-flash
```

Output, repetitive parts omitted:

```
Executing action: bootloader-flash
Serial port COM4
Detecting chip type... ESP32-S3
Bootloader binary size 0x5750 bytes. 0xa8b0 bytes (66%) free.
esptool.py v4.8.1
Chip is ESP32-S3 (QFN56) (revision v0.2)
Features: WiFi, BLE, Embedded PSRAM 8MB (AP_3v3)
Crystal is 40MHz
Changing baud rate to 460800
Configuring flash size...
Flash will be erased from 0x00000000 to 0x00005fff...
SHA digest in image updated
Compressed 22352 bytes to 14245...
Writing at 0x00000000... (100 %)
Wrote 22352 bytes (14245 compressed) at 0x00000000 in 0.6 seconds (effective 298.6 kbit/s)...
Hash of data verified.
```

For a full day I was flashing and flashing, changing configuration in ESP-IDF project, but once flashing with
`espflash`, the ouput told that the older bootloader was still being used.

Finally it turned out that espflash has its own bootloader provided when not using esp-idf-sys package in the project.

```
runner = "espflash flash --monitor --bootloader kukkuu/build/bootloader/bootloader.bin --partition-table partition-table.bin"
```

Now the output from `cargo +esp run --release` is

```
ESP-ROM:esp32s3-20210327
Build:Mar 27 2021
rst:0x1 (POWERON),boot:0xb (SPI_FAST_FLASH_BOOT)
SPIWP:0xee
mode:DIO, clock div:2
load:0x3fce2820,len:0x16ec
load:0x403c8700,len:0x4
load:0x403c8704,len:0xeac
load:0x403cb700,len:0x3170
entry 0x403c894c
I (31) boot: ESP-IDF v5.4-dirty 2nd stage bootloader
I (31) boot: Multicore bootloader
I (31) boot: chip revision: v0.2
I (31) boot: efuse block revision: v1.3
I (34) qio_mode: Enabling default flash chip QIO
I (38) boot.esp32s3: Boot SPI Speed : 40MHz
I (42) boot.esp32s3: SPI Mode       : QIO
I (46) boot.esp32s3: SPI Flash Size : 4MB
I (50) boot: Enabling RNG early entropy source...
I (54) boot: Partition Table:
I (57) boot: ## Label            Usage          Type ST Offset   Length
I (63) boot:  0 nvs              WiFi data        01 02 00009000 00010000
I (70) boot:  1 phy_init         RF data          01 01 00019000 00010000
I (76) boot:  2 factory          factory app      00 00 00030000 00300000
I (83) boot: End of partition table
I (86) boot: Loaded app from partition at offset 0x30000
I (91) boot: Disabling RNG early entropy source...
```

### Debug symbols missing in dev profile

```
xtensa-esp32-elf-readelf -S target/xtensa-esp32s3-none-elf/debug/mainichikatarenshu
There are 4 section headers, starting at offset 0x60:

Section Headers:
  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al
  [ 0]                   NULL            00000000 000000 000000 00      0   0  0
  [ 1] .symtab           SYMTAB          00000000 000034 000010 10      2   1  4
  [ 2] .strtab           STRTAB          00000000 000044 000001 00      0   0  1
  [ 3] .shstrtab         STRTAB          00000000 000045 00001b 00      0   0  1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), p (processor specific)
```

At this point
idf.py erase-flash
Chip erase completed successfully in 5.0s

to see that there was all the time in use some older partition table, as now there is nothing...

Use menuconfig to set the partion table file as `CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="../partitions.csv"`.

Ok, the bootloader and partition table flashing needs to be done via `idf.py`, and the resulted bootloader used when application is flashed via `espflash`.

Uh, there was nothing coming in the final binary, in both release and dev profiles it was the same, no debug symbols.

Went forward by using the [esp-idf-template](https://github.com/esp-rs/esp-idf-template), but that had immediately problems linking:

```
error: linking with `ldproxy` failed: exit code: 101
  = note: [ldproxy] Running ldproxy
          thread 'main' panicked at C:\Users\Jukka\.cargo\registry\src\index.crates.io-6f17d22bba15001f\ldproxy-0.3.4\src/main.rs:44:13:
          Cannot locate argument '--ldproxy-linker <linker>'
          stack backtrace:
          note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
```

Finally this was solved by having a `build.rs` file with the contents:

```rust
fn main() {
    embuild::espidf::sysenv::output();
}
```

Now that the build is passing, checking to see what kind of symbols there are:

```
xtensa-esp32-elf-readelf -S .\target\xtensa-esp32s3-espidf\release\mainichikatarenshu
There are 37 section headers, starting at offset 0x566f34:

Section Headers:
  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al
  [ 0]                   NULL            00000000 000000 000000 00      0   0  0
  [ 1] .rtc.text         PROGBITS        600fe000 06c000 000100 00  AX  0   0  4
  [ 2] .rtc.force_fast   PROGBITS        600fe100 06c100 000020 00  WA  0   0  4
  [ 3] .rtc_noinit       PROGBITS        50000000 06c120 000000 00   W  0   0  1
  [ 4] .rtc.force_slow   PROGBITS        50000000 06c120 000000 00   W  0   0  1
  [ 5] .rtc_reserved     NOBITS          600fffe8 06cfe8 000018 00  WA  0   0  8
  [ 6] .iram0.vectors    PROGBITS        40374000 01f000 000403 00  AX  0   0  4
  [ 7] .iram0.text       PROGBITS        40374404 01f404 00dd03 00  AX  0   0  4
  [ 8] .dram0.dummy      NOBITS          3fc88000 012000 00a200 00  WA  0   0  1
  [ 9] .dram0.data       PROGBITS        3fc92200 01c200 002954 00  WA  0   0 16
  [10] .noinit           NOBITS          3fc94b54 000000 000000 00  WA  0   0  1
  [11] .dram0.bss        NOBITS          3fc94b58 01eb54 000990 00  WA  0   0  8
  [12] .flash.text       PROGBITS        42000020 02e020 03de38 00  AX  0   0  4
  [13] .flash_rodat[...] NOBITS          3c000020 001020 040000 00  WA  0   0  1
  [14] .flash.appdesc    PROGBITS        3c040020 001020 000100 00   A  0   0 16
  [15] .flash.rodata     PROGBITS        3c040120 001120 0103a8 00  WA  0   0 16
  [16] .ext_ram.dummy    NOBITS          3c000020 001020 05ffe0 00  WA  0   0  1
  [17] .iram0.text_end   NOBITS          40382107 02d107 0000f9 00  WA  0   0  1
  [18] .iram0.data       PROGBITS        40382200 06c120 000000 00   W  0   0  1
  [19] .iram0.bss        PROGBITS        40382200 06c120 000000 00   W  0   0  1
  [20] .dram0.heap_start PROGBITS        3fc954e8 06c120 000000 00   W  0   0  1
  [21] .debug_aranges    PROGBITS        00000000 06c120 007688 00      0   0  8
  [22] .debug_info       PROGBITS        00000000 0737a8 1e52b6 00      0   0  1
  [23] .debug_abbrev     PROGBITS        00000000 258a5e 02c1ef 00      0   0  1
  [24] .debug_line       PROGBITS        00000000 284c4d 11285f 00      0   0  1
  [25] .debug_frame      PROGBITS        00000000 3974ac 00cba0 00      0   0  4
  [26] .debug_str        PROGBITS        00000000 3a404c 10920e 01  MS  0   0  1
  [27] .debug_loc        PROGBITS        00000000 4ad25a 0518f6 00      0   0  1
  [28] .debug_ranges     PROGBITS        00000000 4feb50 02bc40 00      0   0  8
  [29] .debug_line_str   PROGBITS        00000000 52a790 001a9d 01  MS  0   0  1
  [30] .debug_loclists   PROGBITS        00000000 52c22d 00a6d6 00      0   0  1
  [31] .debug_rnglists   PROGBITS        00000000 536903 000240 00      0   0  1
  [32] .comment          PROGBITS        00000000 536b43 000093 01  MS  0   0  1
  [33] .xtensa.info      NOTE            00000000 536bd6 000038 00      0   0  1
  [34] .symtab           SYMTAB          00000000 536c10 013a80 10     35 1988  4
  [35] .strtab           STRTAB          00000000 54a690 01c6cc 00      0   0  1
  [36] .shstrtab         STRTAB          00000000 566d5c 0001d7 00      0   0  1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), p (processor specific)
```

```
xtensa-esp32-elf-readelf -S .\target\xtensa-esp32s3-espidf\debug\mainichikatarenshu
There are 37 section headers, starting at offset 0x10c2050:

Section Headers:
  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al
  [ 0]                   NULL            00000000 000000 000000 00      0   0  0
  [ 1] .rtc.text         PROGBITS        600fe000 120000 000100 00  AX  0   0  4
  [ 2] .rtc.force_fast   PROGBITS        600fe100 120100 000020 00  WA  0   0  4
  [ 3] .rtc_noinit       PROGBITS        50000000 120120 000000 00   W  0   0  1
  [ 4] .rtc.force_slow   PROGBITS        50000000 120120 000000 00   W  0   0  1
  [ 5] .rtc_reserved     NOBITS          600fffe8 120fe8 000018 00  WA  0   0  8
  [ 6] .iram0.vectors    PROGBITS        40374000 034000 000403 00  AX  0   0  4
  [ 7] .iram0.text       PROGBITS        40374404 034404 00f527 00  AX  0   0  4
  [ 8] .dram0.dummy      NOBITS          3fc88000 025000 00ba00 00  WA  0   0  1
  [ 9] .dram0.data       PROGBITS        3fc93a00 030a00 002b9c 00  WA  0   0 16
  [10] .noinit           NOBITS          3fc9659c 000000 000000 00  WA  0   0  1
  [11] .dram0.bss        NOBITS          3fc965a0 03359c 0009e0 00  WA  0   0  8
  [12] .flash.text       PROGBITS        42000020 044020 0dbf46 00  AX  0   0  4
  [13] .flash_rodat[...] NOBITS          3c000020 001020 0e0000 00  WA  0   0  1
  [14] .flash.appdesc    PROGBITS        3c0e0020 001020 000100 00   A  0   0 16
  [15] .flash.rodata     PROGBITS        3c0e0120 001120 023480 00  WA  0   0 16
  [16] .ext_ram.dummy    NOBITS          3c000020 001020 10ffe0 00  WA  0   0  1
  [17] .iram0.text_end   NOBITS          4038392b 04392b 0000d5 00  WA  0   0  1
  [18] .iram0.data       PROGBITS        40383a00 120120 000000 00   W  0   0  1
  [19] .iram0.bss        PROGBITS        40383a00 120120 000000 00   W  0   0  1
  [20] .dram0.heap_start PROGBITS        3fc96f80 120120 000000 00   W  0   0  1
  [21] .debug_aranges    PROGBITS        00000000 120120 031e40 00      0   0  8
  [22] .debug_info       PROGBITS        00000000 151f60 5f371d 00      0   0  1
  [23] .debug_abbrev     PROGBITS        00000000 74567d 03cc90 00      0   0  1
  [24] .debug_line       PROGBITS        00000000 78230d 22eaa0 00      0   0  1
  [25] .debug_frame      PROGBITS        00000000 9b0db0 00f3e8 00      0   0  4
  [26] .debug_str        PROGBITS        00000000 9c0198 52e7b4 01  MS  0   0  1
  [27] .debug_loc        PROGBITS        00000000 eee94c 071c80 00      0   0  1
  [28] .debug_ranges     PROGBITS        00000000 f605d0 0600d8 00      0   0  8
  [29] .debug_line_str   PROGBITS        00000000 fc06a8 001a9d 01  MS  0   0  1
  [30] .debug_loclists   PROGBITS        00000000 fc2145 00a6d6 00      0   0  1
  [31] .debug_rnglists   PROGBITS        00000000 fcc81b 000240 00      0   0  1
  [32] .comment          PROGBITS        00000000 fcca5b 000093 01  MS  0   0  1
  [33] .xtensa.info      NOTE            00000000 fccaee 000038 00      0   0  1
  [34] .symtab           SYMTAB          00000000 fccb28 032100 10     35 2614  4
  [35] .strtab           STRTAB          00000000 ffec28 0c3251 00      0   0  1
  [36] .shstrtab         STRTAB          00000000 10c1e79 0001d7 00      0   0  1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), p (processor specific)
```

Same for both.

It took some time to come back to this project as the Waveshare driver and embedded graphics seemed to be
hard to get working together.
Finally when looking at the diff
https://github.com/caemor/epd-waveshare/compare/v0.5.0...v0.6.0
and
https://github.com/embedded-graphics/embedded-graphics
matcing use cases, got the build to compile with
minilar text styling in a display created from `VarDisplay::new()` instead of `Display2in9::default()`.

That then revealed that even when the display has two colors, they should not be defined as `embedded_graphics::pixelcolor::BinaryColor:::Off`,
but as `epd_waveshare::color::Color::White`.

The text started to appear in the screen but they kept drawing om top of existing ones,
not clearning the screen at any point.

The working module was `epd_waveshare::epd2in9_v2`, but the internal commands might differ, which is why
other similar board configurations were tried out...

The display is using a driver SSD1680, so perhaps should be looking at some other EPD provier than Waveshare crate.

GitHub search revealed
https://github.com/embedded-drivers/epd
and
https://github.com/mbv/ssd1680
to be promising ones, the latter being used at https://github.com/mbv/esp32-ssd1680/blob/main/Cargo.toml

None of the drivers seemed fit, since the commands they used were not matching the ones in Arduino examples by Elecrow

Writing my own driver based on mbv/ssd1680

Replaced package `espflash v3.3.0` with `espflash v4.2.0` (executable `espflash.exe`)

After a long battle of comparing a working Arduino example and the Rust driver, finally got the screen to show something.

## Appendix: Timeline

- **January 6–12, 2025** - ESP-IDF toolchain, second-stage bootloader, missing debug symbols and SPI bring-up (`5566c7a`–`e6117ba`)
- **January 17–28, 2025** - Dependency pinning (`esp-idf-svc`, `epd-waveshare`), vendoring `mbv/ssd1680`, logging added around the activate command (`ec38e25`–`170587e`)
- **February 28, 2025** - Reading the specification against the Arduino command bytes (`61cedde`)
- **October 14, 2025, 11:20 AM** - Driver updates matching C++ implementation
- **October 14, 2025, 12:11 PM** - Critical BUSY pin fix discovered
- **October 14, 2025, 12:15 PM** - Polarity inversion fix applied
- **November 8, 2025, 8:50 AM** - Function usage analysis completed
- **November 8, 2025, 9:02 AM** - Refactoring finished, driver cleaned up
- **March 7, 2026, 12:36 AM** - Great refactoring completed with modular architecture
- **November 6, 2026** - Migration from ESP-IDF to `esp-hal` bare-metal with Embassy networking and atomic flash storage
- **November 20, 2026** - Automated SNTP network time sync, persistent Wi-Fi flash provisioning, spot electricity price retrieval, Supabase cloud sync, multi-button hardware bank, schedule-aware sleep, and e-paper display sleep power management
- **November 20, 2026** - Codebase-wide refactor: `crowpanel-ssd1680` extracted to a workspace crate, `main.rs` and `wireless.rs` split into small functions, decision logic moved into host-tested library modules, dead code removed, six bugs fixed, `missing_docs` enforced and CI hardened
- **November 20, 2026** - Authenticated HTTPS (MbedTLS 3.x TLS 1.2/1.3 with fail-closed X.509 date and SAN checks), `porssisahko.net` v2 15-minute spot pricing, chunked HTTP/1.1 framing, and retry-safe Supabase sync coordination

Total development time for driver: ~2 hours of focused debugging  
Total refactoring time: ~4 hours of systematic improvement  
Total lines of code: ~4,300 lines across 19 application modules, plus the `crowpanel-ssd1680` driver crate  
Automated verification: 94 host tests + doctests (`91.92%` line coverage via `cargo tarpaulin`)

**SD Prompt:** `Project timeline visualization, milestone markers, git commit graph, development progress chart, clean infographic style, professional project management aesthetic, blue and green color scheme`

_This blog post documents the development journey of the [mainichikatarenshu](https://github.com/paazmaya/mainichikatarenshu) project, combining technical documentation created during development into a cohesive narrative. All code examples are from the actual implementation._

**License:** MIT  
**Author:** Jukka Paasonen ([@paazmaya](https://github.com/paazmaya))  
**Hardware:** CrowPanel ESP32 2.9" E-paper Display  
**Language:** Bare-metal Rust (`no_std`) with `esp-hal` & Embassy (previously ESP-IDF)
