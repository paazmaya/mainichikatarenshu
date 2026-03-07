# Building a Rust E-Paper Display Driver: A Development Journey

> From Arduino examples to a working Rust driver for the SSD1680 E-Paper display on ESP32-S3

**Project:** [mainichikatarenshu](https://github.com/paazmaya/mainichikatarenshu) - Daily kata training reminder using E-ink display  
**Hardware:** CrowPanel ESP32 2.9" E-paper HMI Display (128×296, SSD1680 driver)  
**Language:** Rust with ESP-IDF framework

---

## Introduction: The Vision

The goal was simple yet ambitious: create a daily martial arts kata reminder using an E-ink display powered by ESP32-S3, written entirely in Rust. The display would wake up at 7:00 AM, show a random kata name, and track whether the training was completed. The data would sync to Google Drive for long-term tracking.

**SD Prompt:** `A minimalist e-paper display showing Japanese kanji characters for martial arts kata, mounted on a wooden desk with morning sunlight, black and white aesthetic, clean modern design, photorealistic, 4k`

---

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

---

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

---

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

---

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

---

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

---

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

---

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

---

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

---

## Technical Achievements

### Driver Architecture

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
├── display_tests.rs # Comprehensive test suite
└── main.rs         # Simplified main application
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

---

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

---

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

---

## What's Next

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

---

## Conclusion

Building a Rust driver for SSD1680 E-paper display was a journey through hardware quirks, toolchain battles, and careful code archaeology. The key was patience—both in waiting for the BUSY pin and in methodically comparing working implementations.

The result is a robust, well-documented driver that handles the specific characteristics of the CrowPanel ESP32 E-paper display. Every bug became a learning opportunity, every fix was documented, and unused code remains as a testament to the exploration process.

The March 2026 refactoring transformed the codebase from functional but repetitive to clean, maintainable, and thoroughly tested. The universal text rendering function alone eliminated 8+ repetitive patterns, while the modular architecture ensures future development remains organized and testable.

The project continues toward its goal: a daily kata reminder that combines traditional martial arts practice with modern embedded systems, all written in clean, maintainable Rust.

**毎日型練習して！** (Train kata every day!)

**SD Prompt:** `Zen garden with modern e-paper display showing Japanese calligraphy, fusion of traditional and modern, peaceful atmosphere, black and white aesthetic, minimalist composition, inspirational technology meets tradition concept`

---

## Appendix: Timeline

- **October 14, 2025, 11:20 AM** - Driver updates matching C++ implementation
- **October 14, 2025, 12:11 PM** - Critical BUSY pin fix discovered
- **October 14, 2025, 12:15 PM** - Polarity inversion fix applied
- **November 8, 2025, 8:50 AM** - Function usage analysis completed
- **November 8, 2025, 9:02 AM** - Refactoring finished, driver cleaned up
- **March 7, 2026, 12:36 AM** - Great refactoring completed with modular architecture

Total development time for driver: ~2 hours of focused debugging  
Total refactoring time: ~4 hours of systematic improvement  
Total lines of code: ~2,500 lines across 10 modules  
Total documentation: 6 markdown files, 700+ lines

**SD Prompt:** `Project timeline visualization, milestone markers, git commit graph, development progress chart, clean infographic style, professional project management aesthetic, blue and green color scheme`

---

_This blog post documents the development journey of the [mainichikatarenshu](https://github.com/paazmaya/mainichikatarenshu) project, combining technical documentation created during development into a cohesive narrative. All code examples are from the actual implementation._

**License:** MIT  
**Author:** Jukka Paasonen ([@paazmaya](https://github.com/paazmaya))  
**Hardware:** CrowPanel ESP32 2.9" E-paper Display  
**Language:** Rust with ESP-IDF framework
