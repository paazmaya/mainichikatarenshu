//! Pin definitions for the SSD1680 e-paper display and associated peripherals
//!
//! This module contains all GPIO pin assignments used in the hardware configuration.

/// Pin configuration constants for the SSD1680 display and peripherals
pub struct Pins;

#[allow(dead_code)]
impl Pins {
    // SPI Display pins
    /// Chip Select pin for SPI display
    pub const CS: u8 = 45;
    /// Data/Command control pin (High for data, Low for command)
    pub const DC: u8 = 46;
    /// Reset pin for display
    pub const RST: u8 = 47;
    /// Busy status pin (High when display is busy)
    pub const BSY: u8 = 48;
    /// SPI Clock pin
    pub const SCK: u8 = 12;
    /// SPI Master Out Slave In
    pub const MOSI: u8 = 11;
    /// SPI Master In Slave Out
    pub const MISO: u8 = 10;

    // Button pins
    /// Exit button
    pub const BTN_EXIT: u8 = 1;
    /// Menu button
    pub const BTN_MENU: u8 = 2;
    /// Up button
    pub const BTN_UP: u8 = 6;
    /// Down button
    pub const BTN_DOWN: u8 = 4;
    /// Confirm button
    pub const BTN_CONF: u8 = 5;
    /// Reset button
    pub const BTN_RESET: u8 = 3;

    // Other pins
    /// Power LED indicator
    pub const PIN_POWER_LED: u8 = 41;

    // TF Card (SD Card) pins
    /// TF Card Chip Select
    pub const TFC_CS: u8 = 10;
    /// TF Card Master Out Slave In
    pub const TFC_MOSI: u8 = 40;
    /// TF Card Master In Slave Out
    pub const TFC_MISO: u8 = 13;
    /// TF Card Clock
    pub const TFC_CLK: u8 = 39;
}
