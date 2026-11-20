//! SSD1680 e-paper driver for the
//! [CrowPanel ESP32 2.9" E-paper HMI Display](https://www.elecrow.com/crowpanel-esp32-2-9-e-paper-hmi-display-with-128-296-resolution-black-white-color-driven-by-spi-interface.html)
//! (128×296 pixels, black and white, SPI).
//!
//! The crate is `no_std`, does not allocate and builds on the
//! [`embedded-hal`](https://docs.rs/embedded-hal) 1.0 traits, so it works with any HAL.
//!
//! ## Usage
//!
//! 1. Draw into a [`Display2in9`] frame buffer with
//!    [`embedded-graphics`](https://docs.rs/embedded-graphics). `BinaryColor::On` is black
//!    and `BinaryColor::Off` is white.
//! 2. Send the buffer to the panel with [`Ssd1680::display_frame`]. It initialises the
//!    controller, writes the frame, refreshes the panel and waits for BUSY to go low.
//! 3. Call [`Ssd1680::sleep`] between updates. The next [`Ssd1680::display_frame`] wakes
//!    the controller again with a hardware reset.
//!
//! ```rust,ignore
//! use crowpanel_ssd1680::{Display, Display2in9, Ssd1680};
//! use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
//!
//! let mut panel = Ssd1680::new(spi, busy, dc, rst, delay)?;
//! let mut frame = Display2in9::new();
//! frame.clear(BinaryColor::Off)?;
//! // ... draw with embedded-graphics ...
//! panel.display_frame(frame.buffer())?;
//! panel.sleep()?;
//! ```
//!
//! ## References
//!
//! - SSD1680 datasheet, Rev 0.14 (section 9.1, "General operation flow to drive display panel")
//! - [Elecrow CrowPanel examples](https://github.com/Elecrow-RD/CrowPanel-ESP32-E-Paper)
//! - Buffer handling is derived from [mbv/ssd1680](https://github.com/mbv/ssd1680)
#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod cmd;
pub mod color;
pub mod driver;
pub mod flag;
pub mod graphics;
mod interface;
pub mod pins;

pub use cmd::Cmd;
pub use color::Color;
pub use display_interface::DisplayError;
pub use driver::Ssd1680;
pub use flag::Flag;
pub use graphics::{Display, Display2in9, DisplayRotation};
pub use pins::Pins;

/// Panel width in pixels (source lines).
pub const WIDTH: u16 = 128;

/// Panel height in pixels (gate lines).
pub const HEIGHT: u16 = 296;

/// Size of one full frame in bytes, one bit per pixel.
pub const FRAME_BYTES: usize = (WIDTH as usize).div_ceil(8) * HEIGHT as usize;
