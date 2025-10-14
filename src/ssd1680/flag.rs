/// Various flags and constants used in the SSD1680 e-paper display driver.
///
/// This struct contains all the flag values and constants for configuring the SSD1680 controller.
/// All constants are documented inline with their respective values.
pub struct Flag;
#[allow(missing_docs)]
impl Flag {
    pub const DISPLAY_MODE_1: u8 = 0xF7;

    // Driver Output Control (0x01) flags
    pub const DRIVER_OUTPUT_GATE_SCAN_FROM_G0: u8 = 0x00;
    pub const DRIVER_OUTPUT_GATE_SCAN_FROM_GN: u8 = 0x01;
    pub const DRIVER_OUTPUT_SOURCE_NORMAL_COLOR: u8 = 0x00;
    pub const DRIVER_OUTPUT_SOURCE_INVERSE_COLOR: u8 = 0x02;

    // Data Entry Mode (0x11) flags
    pub const DATA_ENTRY_DECRY_DECRX: u8 = 0x00; // Y decrement, X decrement
    pub const DATA_ENTRY_DECRY_INCRX: u8 = 0x01; // Y decrement, X increment
    pub const DATA_ENTRY_INCRY_DECRX: u8 = 0x02; // Y increment, X decrement
    pub const DATA_ENTRY_INCRY_INCRX: u8 = 0x03; // Y increment, X increment

    // Deep Sleep Mode (0x10) flags
    pub const DEEP_SLEEP_NORMAL_MODE: u8 = 0x00;
    pub const DEEP_SLEEP_MODE_1: u8 = 0x01; // Enter deep sleep mode

    // Temperature Sensor Control (0x18) flags
    pub const INTERNAL_TEMP_SENSOR: u8 = 0x80;
    pub const EXTERNAL_TEMP_SENSOR: u8 = 0x00;

    // Display Update Control 1 (0x21) flags
    pub const DISPLAY_UPDATE_BW_RAM: u8 = 0x01; // Use RAM 0x24 (black/white)
    pub const DISPLAY_UPDATE_RED_RAM: u8 = 0x02; // Use RAM 0x26 (red)
    pub const DISPLAY_UPDATE_BOTH_RAMS: u8 = 0x03; // Use both RAM 0x24 and 0x26

    // Common display update sequences (combinations of above)
    pub const DISPLAY_UPDATE_FULL: u8 = 0xF4; // Full update
    pub const DISPLAY_UPDATE_FAST: u8 = 0xB1; // Fast update
    pub const DISPLAY_UPDATE_PARTIAL_1: u8 = 0x91; // Partial update option 1
    pub const DISPLAY_UPDATE_PARTIAL_2: u8 = 0xC7; // Partial update option 2

    // Display Update Control 2 (0x22) flags
    // Display Options (bits 7:4)
    pub const DISPLAY_OPTION_NORMAL: u8 = 0x00; // Normal mode
    pub const DISPLAY_OPTION_BYPASS_RAM: u8 = 0x10; // Bypass RAM, ignore RAM content

    // Sequence Options (bits 3:0)
    pub const SEQUENCE_CLOCK_ONLY: u8 = 0x00; // Clock only
    pub const SEQUENCE_CLOCK_ANALOG: u8 = 0x01; // Clock & analog
    pub const SEQUENCE_CLOCK_ANALOG_TEMP: u8 = 0x02; // Clock, analog, load temp
    pub const SEQUENCE_CLOCK_ANALOG_TEMP_LUT: u8 = 0x03; // Clock, analog, load temp, load LUT
    pub const SEQUENCE_ALL_WITH_DISPLAY_REFRESH: u8 = 0x04; // All, including display refresh

    // VCI Detection (0x15) flags
    pub const VCI_DETECTION_DISABLE: u8 = 0x00;
    pub const VCI_DETECTION_ENABLE: u8 = 0x01;

    // Border Waveform Control (0x3C) flags
    pub const BORDER_WAVEFORM_BLACK: u8 = 0x00; // Black border
    pub const BORDER_WAVEFORM_WHITE: u8 = 0x01; // White border
    pub const BORDER_WAVEFORM_RED: u8 = 0x02; // Red border (for tri-color displays)
    pub const BORDER_WAVEFORM_FOLLOW_LUT: u8 = 0x04;
    pub const BORDER_WAVEFORM_LUT1: u8 = 0x01;
    pub const BORDER_WAVEFORM_FLOATING: u8 = 0x05; // Floating (last frame value)

    // Fixed high bits for border waveform control
    pub const BORDER_WAVEFORM_FIXED_BITS: u8 = 0x50; // Bits 6:4 fixed at 0101b

    // Status Bit Read (0x2F) flag bits
    pub const STATUS_BUSY_BIT: u8 = 0x01; // D0: 0=Ready, 1=Busy
    pub const STATUS_OPERATION_BIT: u8 = 0x02; // D1: 0=No operation, 1=Operation in progress
    pub const STATUS_DISPLAY_BIT: u8 = 0x04; // D2: 0=Normal, 1=In update
    pub const STATUS_HV_READY_BIT: u8 = 0x08; // D3: HV ready status
    pub const STATUS_VCOM_SENSE_BIT: u8 = 0x10; // D4: VCOM sense complete
    pub const STATUS_LUT_OPERATION_BIT: u8 = 0x20; // D5: LUT operation result
    pub const STATUS_TEMP_READ_BIT: u8 = 0x40; // D6: Temperature read complete
    pub const STATUS_POWER_BIT: u8 = 0x80; // D7: Power status

    // Gate Driving Voltage Control (0x03) default values
    pub const GATE_VOLTAGE_VGH_DEFAULT: u8 = 0x19; // Default: 15.0V (10V + 25 * 0.2V)
    pub const GATE_VOLTAGE_VGL_DEFAULT: u8 = 0x03; // Default: -10.6V (-10V - 3 * 0.2V)

    // Source Driving Voltage Control (0x04) default values
    pub const SOURCE_VOLTAGE_VDH_DEFAULT: u8 = 0x0C; // Default: 3.6V (2.4V + 12 * 0.1V)
    pub const SOURCE_VOLTAGE_VDL_DEFAULT: u8 = 0x0C; // Default: 1.4V (0.2V + 12 * 0.1V)
    pub const SOURCE_VOLTAGE_VDHR_DEFAULT: u8 = 0x03; // Default: 2.7V (2.4V + 3 * 0.1V)

    // Booster Soft Start Control (0x0C) default values
    pub const BOOSTER_SOFT_START_PHASE1_DEFAULT: u8 = 0x17;
    pub const BOOSTER_SOFT_START_PHASE2_DEFAULT: u8 = 0x17;
    pub const BOOSTER_SOFT_START_PHASE3_DEFAULT: u8 = 0x17;

    // VCOM Control (0x2B) default value
    pub const VCOM_DEFAULT: u8 = 0x28; // Default: -2.0V (-0.1V - 0x28 * 0.05V)

    // LUT mode selection constants
    pub const LUT_MODE_FULL_UPDATE: u8 = 0x00;
    pub const LUT_MODE_FAST_UPDATE: u8 = 0x01;
    pub const LUT_MODE_PARTIAL_UPDATE: u8 = 0x02;

    // Auto Write pattern constants (for 0x46 and 0x47)
    pub const AUTO_WRITE_PATTERN_ALL_WHITE: u8 = 0xFF;
    pub const AUTO_WRITE_PATTERN_ALL_BLACK: u8 = 0x00;
    pub const AUTO_WRITE_PATTERN_CHECKERBOARD1: u8 = 0xAA;
    pub const AUTO_WRITE_PATTERN_CHECKERBOARD2: u8 = 0x55;

    // Common RAM data bit definitions (for WRITE_BW_DATA and WRITE_RED_DATA)
    pub const RAM_BIT_BLACK: u8 = 0x00;
    pub const RAM_BIT_WHITE: u8 = 0x01;
    pub const RAM_BIT_RED: u8 = 0x01; // For red RAM (0x26), bit=1 means red

    // OTP Program Mode (0x39) flags
    pub const OTP_PROGRAM_MODE_DISABLE: u8 = 0x00;
    pub const OTP_PROGRAM_MODE_ENABLE: u8 = 0x01;
}

/*
Arduino example code had these:
0x01 - Border Waveform Control (Follow LUT, LUT1)
0x03 - Data Entry Mode (Increment Y, Increment X)
0x80 - Internal Temperature Sensor
*/
