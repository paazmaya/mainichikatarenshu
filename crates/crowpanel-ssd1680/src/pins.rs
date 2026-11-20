//! GPIO assignments of the CrowPanel ESP32 2.9" E-paper HMI Display.
//!
//! Taken from the board schematic. These are plain GPIO numbers; wiring them into the
//! HAL is up to the application.

/// GPIO numbers of the CrowPanel ESP32-S3 board.
pub struct Pins;

impl Pins {
    /// E-paper SPI chip select
    pub const CS: u8 = 45;
    /// E-paper data/command select (high for data, low for command)
    pub const DC: u8 = 46;
    /// E-paper reset (active low)
    pub const RST: u8 = 47;
    /// E-paper BUSY output (high while the controller is busy)
    pub const BUSY: u8 = 48;
    /// E-paper SPI clock
    pub const SCK: u8 = 12;
    /// E-paper SPI data out of the MCU
    pub const MOSI: u8 = 11;
    /// SPI data into the MCU (not used by the write-only panel)
    pub const MISO: u8 = 10;
    /// Power enable for the e-paper panel (`LCD_3.3_CTL` in the schematic)
    pub const EPD_POWER: u8 = 7;

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

    /// Power LED
    pub const POWER_LED: u8 = 41;

    /// TF card chip select
    pub const TF_CS: u8 = 10;
    /// TF card data out of the MCU
    pub const TF_MOSI: u8 = 40;
    /// TF card data into the MCU
    pub const TF_MISO: u8 = 13;
    /// TF card clock
    pub const TF_CLK: u8 = 39;
    /// Power enable for the TF card (`TF_3.3_CTL` in the schematic)
    pub const TF_POWER: u8 = 42;
}
