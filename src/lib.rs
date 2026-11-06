//! Library interface for mainichikatarenshu
//!
//! This provides the public API for testing without requiring ESP32 hardware.

#![cfg_attr(not(feature = "test-utils"), no_std)]

#[cfg(not(feature = "test-utils"))]
extern crate alloc;

#[cfg(feature = "test-utils")]
extern crate alloc;

// Re-export the main modules for testing
pub mod buttons;
pub mod kata_display;
pub mod network_config;
pub mod partitions;
pub mod schedule;
pub mod ssd1680;
pub mod storage;
pub mod training;
pub mod wpa2;

/// Kata names validated and embedded from the optional `kata.yaml` at build time.
pub mod kata {
    include!(concat!(env!("OUT_DIR"), "/kata.rs"));
}

// Re-export main types for easier testing
pub use kata_display::TrainingStats;
pub use ssd1680::color::Color;
pub use ssd1680::graphics::Display2in13;
pub use ssd1680::text::{TextAlignment, TextConfig, TextRenderer};
