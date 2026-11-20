//! Hardware-independent logic of the mainichikatarenshu kata reminder.
//!
//! The firmware binary (`src/main.rs`) wires these modules to the ESP32-S3
//! peripherals. Everything here builds for the host with the `test-utils`
//! feature, so it can be unit tested without hardware.

#![cfg_attr(not(feature = "test-utils"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

pub mod buttons;
pub mod clock;
pub mod console;
pub mod electricity;
pub mod http;
pub mod layout;
pub mod network_config;
pub mod partitions;
pub mod schedule;
pub mod screens;
pub mod sntp;
pub mod storage;
pub mod supabase;
pub mod text;
#[cfg(any(feature = "wifi", feature = "test-utils"))]
pub mod tls;
pub mod training;

/// Kata names validated and embedded from the optional `kata.yaml` at build time.
pub mod kata {
    include!(concat!(env!("OUT_DIR"), "/kata.rs"));
}

pub use crowpanel_ssd1680::{Color, Display2in9};
pub use text::{TextAlignment, TextConfig, TextRenderer};
