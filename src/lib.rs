//! Library interface for mainichikatarenshu
//!
//! This provides the public API for testing without requiring ESP32 hardware.

#![cfg_attr(not(feature = "test-utils"), no_std)]

#[cfg(not(feature = "test-utils"))]
extern crate alloc;

#[cfg(feature = "test-utils")]
extern crate alloc;

// Re-export the main modules for testing
pub mod ssd1680;
pub mod kata_display;

// Only include ESP32-specific modules when not in test mode
#[cfg(not(feature = "test-utils"))]
pub mod input;
#[cfg(not(feature = "test-utils"))]
pub mod wifi;

#[cfg(not(feature = "test-utils"))]
mod app; // Private main module (renamed from main.rs)

// Re-export main types for easier testing
pub use ssd1680::graphics::Display2in13;
pub use ssd1680::text::{TextRenderer, TextConfig, TextAlignment};
pub use ssd1680::color::Color;
pub use kata_display::TrainingStats;
