//! SSD1680 E-Paper Display Driver for CrowPanel ESP32 2.9" Display
//!
//! This driver is specifically designed for the [CrowPanel ESP32 2.9" E-paper HMI Display](https://www.elecrow.com/crowpanel-esp32-2-9-e-paper-hmi-display-with-128-296-resolution-black-white-color-driven-by-spi-interface.html)
//! with 128×296 resolution, black and white color, driven by SPI interface.
//!
//! ## Hardware Specifications
//!
//! - **Display:** 2.9-inch E-Paper, 128×296 resolution
//! - **Controller:** SSD1680
//! - **Colors:** Black and White (monochrome)
//! - **Interface:** SPI
//! - **MCU:** ESP32-S3 (240MHz, 8MB flash)
//! - **Refresh Time:** ~2 seconds (full update)
//!
//! ## Key Features
//!
//! - **Hardware-specific implementation** - Commands and sequences match the working Arduino examples
//! - **Polarity handling** - Correctly handles the inverted polarity of this specific display
//! - **BUSY pin management** - Proper waiting for display updates to complete
//! - **embedded-graphics integration** - Compatible with the embedded-graphics ecosystem
//!
//! ## Important Hardware Quirks
//!
//! This display has **inverted polarity** compared to typical expectations:
//! - `0x00` byte = white pixels ⚪
//! - `0xFF` byte = black pixels ⚫
//!
//! For `embedded-graphics`:
//! - `BinaryColor::Off` (0) → **white** on this display
//! - `BinaryColor::On` (1) → **black** on this display
//!
//! ## Usage
//!
//! This driver exposes the raw buffer interface. To display something:
//!
//! 1. Create a buffer and draw things onto it using [`embedded_graphics`](https://github.com/jamwaffles/embedded-graphics)
//! 2. Send the buffer to the display using [`driver::Ssd1680::write_buffer_and_update`]
//! 3. The driver handles the update sequence and waits for the BUSY pin
//!
//! ## Example
//!
//! ```rust,no_run
//! use embedded_graphics::{prelude::*, pixelcolor::BinaryColor, text::Text};
//! use ssd1680::{Ssd1680, graphics::Display2in13};
//!
//! // Initialize driver (see main.rs for full setup)
//! let mut ssd1680 = Ssd1680::new(spi, busy_pin, dc_pin, rst_pin, delay)?;
//! ssd1680.cpp_init()?;
//!
//! // Create display buffer
//! let mut display = Display2in13::new();
//! display.clear(BinaryColor::Off)?; // Clear to white
//!
//! // Draw text (BinaryColor::On = black pixels)
//! Text::new("Hello!", Point::new(10, 10), text_style)
//!     .draw(&mut display)?;
//!
//! // Send to display
//! ssd1680.write_buffer_and_update(display.buffer())?;
//! ```
//!
//! ## References
//!
//! - [SSD1680 Datasheet](https://www.good-display.com/companyfile/32.html)
//! - [Elecrow Arduino Examples](https://github.com/Elecrow-RD/CrowPanel-ESP32-E-Paper)
//! - Based on [mbv/ssd1680](https://github.com/mbv/ssd1680) structure
//!
//!
#![no_std]
#![deny(missing_docs)]
#![allow(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]

pub mod color;
pub mod driver;

pub mod cmd;
pub mod flag;
pub mod graphics;
pub mod pins;

/// Maximum display height this driver supports
#[allow(dead_code)]
pub const MAX_HEIGHT: u16 = 296;

/// Maximum display width this driver supports
#[allow(dead_code)]
pub const MAX_WIDTH: u16 = 176;

/// Display height, pixels vertically
pub const HEIGHT: u16 = 296;

/// Display width, pixels horizontally
pub const WIDTH: u16 = 128;

pub mod interface;
