# Building a Rust E-Paper Display Driver: A Development Journey

> From Arduino examples to a working Rust driver for the SSD1680 E-Paper display on ESP32-S3

**Project:** [mainichikatarenshu](https://github.com/paazmaya/mainichikatarenshu) - Daily kata training reminder using E-ink display  
**Hardware:** CrowPanel ESP32 2.9" E-paper HMI Display (128×296, SSD1680 driver)  
**Language:** Bare-metal Rust (`no_std`) with `esp-hal` & Embassy (previously ESP-IDF)



## Introduction: The Vision

The goal was simple yet ambitious: create a daily martial arts kata reminder using an E-ink display powered by ESP32-S3, written entirely in Rust. The display would wake up at 7:00 AM, show a random kata name, and track whether the training was completed. The data would sync to Google Drive for long-term tracking.

**SD Prompt:** `A minimalist e-paper display showing Japanese kanji characters for martial arts kata, mounted on a wooden desk with morning sunlight, black and white aesthetic, clean modern design, photorealistic, 4k`



## Chapter 1: The Toolchain Odyssey (October 2025)

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

**SD Prompt:** `A computer screen showing terminal windows with compilation logs, ESP32 development board connected via USB cable, dim workshop lighting, technical atmosphere, developer workspace, cinematic lighting, detailed electronics`



## Chapter 2: The Driver Search (October 14, 2025 - Morning)

### Existing Drivers Don't Fit

Initially, I tried using existing Rust drivers:

- **epd-waveshare**: The `epd2in9_v2` module worked partially, but commands didn't match
- **embedded-graphics**: Integration was complex, colors weren't mapping correctly
- **mbv/ssd1680**: Promising but commands didn't align with the Elecrow Arduino examples

The display uses an SSD1680 controller, but the command sequences in the [working Arduino examples](https://github.com/Elecrow-RD/CrowPanel-ESP32-E-Paper) differed from all existing Rust drivers. The decision was made: **write a custom driver from scratch**.

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

### 5. Keep Debug Code

Functions marked `**UNUSED**` aren't waste—they're debugging tools and future features. Test patterns, emergency clears, and alternative update modes proved invaluable during development.

### 6. Invest in Abstraction

The universal text function demonstrates the power of good abstraction. One function handling any text length with automatic multiline eliminated 8+ repetitive patterns and made the codebase dramatically more maintainable.

### 7. Separate Concerns Early

Breaking code into logical modules (text rendering, display utilities, application logic, testing) from the start prevents technical debt. Each module has a single responsibility and clear interfaces.

### 8. Testability is Design

Designing for testability isn't an afterthought—it's fundamental. Pure functions that accept generic traits make comprehensive testing possible and catch bugs early.

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
runner = "espflash flash --monitor"
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

With `espflash` managing partition tables and flashing directly, the helper C project `kukkuu/` and `sdkconfig.defaults` were retired and removed. Tooling configurations (`wokwi.toml`, VS Code launch profiles) were updated to target the `xtensa-esp32s3-none-elf` binary.

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

| New / reshaped module | What moved there |
|---|---|
| `src/clock.rs` | RTC timestamp validation; replaces three copies of the magic `946_684_800` |
| `src/console.rs` | `LineBuffer` (byte accumulation, overflow, non-ASCII) and a `Command` parser |
| `src/screens.rs` | Pure `select_screen()` returning a `Screen` enum, plus `draw_screen()` |
| `src/layout.rs` | Former `DisplayManager` unit struct, now free `draw_*` functions |
| `src/http.rs` | HTTP status parsing shared by the price and Supabase clients |
| `training::choose_kata` | Unbiased rejection sampling over the kata list |
| `ButtonBank::pressed` | Samples all buttons at once and returns new presses |

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

### Guard Rails

- `#![warn(missing_docs)]` on the library, with every public item documented.
- The `write_text` doctest, which had never compiled, now runs in CI.
- CI runs clippy with `-D warnings` on host tests and on both firmware feature sets.
- Local `cargo +stable tarpaulin` enforces the 90% coverage gate configured in `tarpaulin.toml`.

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

Inspecting the live `porssisahko.net` documentation confirmed that Finland's spot market now runs on 15-minute (*varttisähkö*) intervals under **API v2** (`GET /v2/price.json?date=YYYY-MM-DDTHH:MM:SS.000Z`). Switching from `/v1/price.json` to `/v2/price.json` reports the actual current quarter-hour price in `c/kWh` (VAT included) and queries in UTC (`Z`), eliminating the repeated local-hour ambiguity during the autumn DST fallback.

At the protocol boundary:

- `src/http.rs` validates status codes and headers, bounds buffer capacity (2 KiB for Cloudflare header headroom), decodes `Transfer-Encoding: chunked` in place, stops immediately when self-delimited responses (`Content-Length`, `chunked`, `204`/`304`) complete, and rejects ambiguous (`Transfer-Encoding` + `Content-Length`), truncated, or oversized responses.
- `src/electricity.rs` parses `{"price": ...}` with `serde_json` rather than scanning raw response text, preserving integer tenths-of-a-cent rounding, and `PriceUpdates` only marks a 15-minute slot complete once the display channel accepts the update.
- `src/supabase.rs` extracts `RecordSync` and `OutcomeDelivery` so uploads wait for a valid clock, back off for 60 seconds after failures instead of retrying every poll cycle, and retain `SyncOutcome` across channel saturation rather than dropping an acknowledgement or wedging `in_flight`.

We also evaluated extracting `src/electricity.rs` into a second workspace crate alongside `crates/crowpanel-ssd1680` and chose to defer: unlike the display driver, the price module is compact (~130 lines of logic), shares this firmware's display formatting and 15-minute refresh state, and has no second consumer yet.

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



## Appendix: Timeline

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
