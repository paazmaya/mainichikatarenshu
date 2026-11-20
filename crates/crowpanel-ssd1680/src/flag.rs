//! Parameter values sent with the SSD1680 commands in [`Cmd`](crate::Cmd).
//!
//! Only the values this panel needs are defined. See the SSD1680 datasheet command table
//! for all options.

/// Command parameter values used by the driver.
pub struct Flag;

impl Flag {
    /// Data Entry Mode (0x11): X and Y address counters increment, X first.
    pub const DATA_ENTRY_INCRY_INCRX: u8 = 0x03;

    /// Deep Sleep Mode (0x10): enter deep sleep mode 1, keeping RAM contents.
    ///
    /// Leaving deep sleep requires a hardware reset.
    pub const DEEP_SLEEP_MODE_1: u8 = 0x01;

    /// Temperature Sensor Control (0x18): use the internal temperature sensor.
    pub const INTERNAL_TEMP_SENSOR: u8 = 0x80;

    /// Border Waveform Control (0x3C): border follows LUT1 (white).
    pub const BORDER_WAVEFORM_LUT1: u8 = 0x01;

    /// Display Update Control 2 (0x22): full refresh used by this panel.
    ///
    /// Enables clock and analog, loads the temperature value and waveform LUT from OTP,
    /// drives the display, then disables analog and clock. The commonly quoted `0xC7`
    /// does not refresh this panel correctly.
    pub const DISPLAY_UPDATE_FULL: u8 = 0xF4;
}
