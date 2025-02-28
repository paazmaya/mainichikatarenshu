pub struct Cmd;
impl Cmd {
    /// Driver Output Control
    pub const DRIVER_CONTROL: u8 = 0x01;
    /// Gate Driving voltage Control
    pub const GATE_VOLTAGE_CONTROL: u8 = 0x03;
    /// Source Driving voltage Control
    pub const SOURCE_VOLTAGE_CONTROL: u8 = 0x04;
    /// Initial Code Setting OTP Program
    pub const INIT_CODE_SETTING_OTP: u8 = 0x08;
    /// Write Register for Initial Code Setting
    pub const WRITE_INIT_CODE_SETTING: u8 = 0x09;
    /// Read Register for Initial Code Setting
    pub const READ_INIT_CODE_SETTING: u8 = 0x0A;
    /// Booster Soft start Control
    pub const BOOST_SOFT_START_CONTROL: u8 = 0xC;
    /// Deep Sleep mode
    pub const DEEP_SLEEP_MODE: u8 = 0x10;
    /// Data Entry mode setting
    pub const DATA_ENTRY_MODE: u8 = 0x11;
    /// SW reset, During operation, BUSY pad will output high.
    pub const SW_RESET: u8 = 0x12;
    /// HV Ready Detection
    pub const HV_READY: u8 = 0x14;
    /// VCI Detection
    pub const VCI_DETECTION: u8 = 0x15;
    /// Temperature Sensor Control
    pub const TEMP_CONTROL: u8 = 0x18;
    // Additionally: Write 1A, read 1B, write external 1C

    /// Master Activation. Activate Display Update Sequence
    pub const MASTER_ACTIVATE: u8 = 0x20;
    /// Display Update Control 1
    pub const DISPLAY_UPDATE_CTRL1: u8 = 0x21;
    /// Display Update Control 2
    pub const DISPLAY_UPDATE_CTRL2: u8 = 0x22;
    /// Write RAM (Black White) / RAM 0x24
    pub const WRITE_BW_DATA: u8 = 0x24;
    ///  Write RAM (RED) / RAM 0x26
    pub const WRITE_RED_DATA: u8 = 0x26;
    /// Read RAM. After this command, data read on the MCU bus will fetch data from RAM
    pub const READ_DATA: u8 = 0x27;
    /// VCOM Sense
    pub const VCOM_SENSE: u8 = 0x28;
    /// VCOM Sense Duration
    pub const VCOM_SENSE_DURATION: u8 = 0x29;
    /// Program VCOM OTP
    pub const PROGRAM_VCOM_OTP: u8 = 0x2A;
    /// Write Register for VCOM Control
    pub const WRITE_VCOM_CONTROL_REGISTER: u8 = 0x2B;
    // Write VCOM register
    pub const WRITE_VCOM_REGISTER: u8 = 0x2C;
    /// OTP Register Read for Display Option
    pub const OTP_REGISTER_READ: u8 = 0x2D;
    /// User ID Read
    pub const USER_ID_READ: u8 = 0x2E;
    /// Status Bit Read
    pub const STATUS_BIT_READ: u8 = 0x2F;
    /// Program WS OTP
    pub const PROGRAM_WS_OTP: u8 = 0x30;
    /// Load WS OTP
    pub const LOAD_WS_OTP: u8 = 0x31;
    /// Write LUT register
    pub const WRITE_LUT_REGISTER: u8 = 0x32;
    /// CRC calculation
    pub const CRC_CALCULATION: u8 = 0x34;
    /// CRC Status Read
    pub const CRC_STATUS_READ: u8 = 0x35;
    /// Program OTP selection
    pub const PROGRAM_OTP_SELECTION: u8 = 0x36;
    /// Write Register for Display Option
    pub const WRITE_REGISTER_FOR_DISPLAY_OPTION: u8 = 0x37;
    /// Write Register for User ID
    pub const WRITE_REGISTER_FOR_USER_ID: u8 = 0x38;
    /// OTP program mode
    pub const OTP_PROGRAM_MODE: u8 = 0x39;
    /// Border Waveform Control
    pub const BORDER_WAVEFORM_CONTROL: u8 = 0x3C;
    /// End Option (EOPT)
    pub const END_OPTION: u8 = 0x3F;
    /// Read RAM Option
    pub const READ_RAM_OPTION: u8 = 0x41;
    /// Set RAM X - address Start / End position
    pub const SET_RAMX_START_END: u8 = 0x44;
    /// Set Ram Y- address Start / End position
    pub const SET_RAMY_START_END: u8 = 0x45;
    /// Auto Write RED RAM for Regular Pattern
    pub const AUTO_WRITE_RED_RAM_FOR_REGULAR_PATTERN: u8 = 0x46;
    /// Auto Write B/W RAM for Regular Pattern
    pub const AUTO_WRITE_BW_RAM_FOR_REGULAR_PATTERN: u8 = 0x47;
    /// Set RAM X address counter
    pub const SET_RAMX_COUNTER: u8 = 0x4E;
    /// Set RAM Y address counter
    pub const SET_RAMY_COUNTER: u8 = 0x4F;
    /// NOP. This command is an empty command; it does not have any effect on the display module. However it can be used to terminate Frame Memory Write or Read Commands.
    pub const NOP: u8 = 0xE3;
}
