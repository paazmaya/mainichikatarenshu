/// SSD1680 command constants
///
/// This struct contains all the command codes for the SSD1680 e-paper display controller.
/// Commands are organized by their function and documented according to the SSD1680 datasheet.
pub struct Cmd;
impl Cmd {
    /// Driver Output Control (0x01)
    ///
    /// Parameters:
    /// - D[7:0]: Scan line setting (MUX 296-1). Default: 27h
    /// - D[8]: Gate scan direction (0: Scan from G0, 1: Scan from G(N-1))
    /// - D[9]: Source shift direction (0: Normal color display, 1: Inverse color display)
    /// - D[10]: No function.
    ///
    /// This command sets the number of gate lines and scan sequence.
    pub const DRIVER_CONTROL: u8 = 0x01;

    /// Gate Driving Voltage Control (0x03)
    ///
    /// Parameters:
    /// - D[5:0]: VGH[5:0], VGH = 10V + VGH[5:0] * 0.2V. Default: 19h (15.0V)
    /// - D[13:8]: VGL[5:0], VGL = -10V - VGL[5:0] * 0.2V. Default: 03h (-10.6V)
    ///
    /// Sets the gate driving voltage levels.
    pub const GATE_VOLTAGE_CONTROL: u8 = 0x03;

    /// Source Driving Voltage Control (0x04)
    ///
    /// Parameters:
    /// - D[3:0]: VDH[3:0], VDH = 2.4V + VDH[3:0] * 0.1V. Default: 0Ch (3.6V)
    /// - D[7:4]: VDL[3:0], VDL = 0.2V + VDL[3:0] * 0.1V. Default: 0Ch (1.4V)
    /// - D[10:8]: VDHR[2:0], VDHR = 2.4V + VDHR[2:0] * 0.1V. Default: 3h (2.7V)
    ///
    /// Sets the source driving voltage levels.
    pub const SOURCE_VOLTAGE_CONTROL: u8 = 0x04;

    /// Initial Code Setting OTP Program (0x08)
    ///
    /// OTP (One Time Programmable) memory programming command.
    /// Used to program the initial code settings into OTP memory.
    pub const INIT_CODE_SETTING_OTP: u8 = 0x08;

    /// Write Register for Initial Code Setting (0x09)
    ///
    /// Writes data to the initial code setting register.
    /// This register contains initialization parameters.
    pub const WRITE_INIT_CODE_SETTING: u8 = 0x09;

    /// Read Register for Initial Code Setting (0x0A)
    ///
    /// Reads data from the initial code setting register.
    pub const READ_INIT_CODE_SETTING: u8 = 0x0A;

    /// Booster Soft Start Control (0x0C)
    ///
    /// Parameters:
    /// - D[7:0]: Phase1 soft-start timing and driving strength. Default: 17h
    /// - D[15:8]: Phase2 soft-start timing and driving strength. Default: 17h
    /// - D[23:16]: Phase3 soft-start timing and driving strength. Default: 17h
    ///
    /// Controls the booster's soft start behavior to ensure proper power initialization.
    pub const BOOST_SOFT_START_CONTROL: u8 = 0xC;

    /// Deep Sleep Mode (0x10)
    ///
    /// Parameters:
    /// - D[0]: Enter deep sleep mode (0: Normal mode, 1: Enter deep sleep mode)
    ///
    /// After this command is initiated, the chip will enter Deep Sleep Mode.
    /// The BUSY pad will keep output high during sleep.
    /// In deep sleep mode, the DC/DC circuit and internal oscillator are disabled.
    /// Exit: Hardware reset is required to exit deep sleep mode.
    pub const DEEP_SLEEP_MODE: u8 = 0x10;

    /// Data Entry Mode Setting (0x11)
    ///
    /// Parameters:
    /// - D[2:0]: Entry mode
    ///   - 00h = Y decrement, X decrement
    ///   - 01h = Y decrement, X increment
    ///   - 02h = Y increment, X decrement
    ///   - 03h = Y increment, X increment
    ///   - Default: 03h
    /// - D[3]: RAM address A[8] mapping to AM, reserved for future use.
    /// - D[7:4]: Reserved
    ///
    /// Sets the RAM data entry mode and address increment/decrement direction.
    pub const DATA_ENTRY_MODE: u8 = 0x11;

    /// Software Reset (0x12)
    ///
    /// Performs a software reset of the controller.
    /// During operation, BUSY pad will output high.
    /// It resets the commands and parameters to their S/W Reset default values
    /// except for Deep Sleep Mode (0x10).
    ///
    /// Note: RAM contents are unaffected by this command.
    pub const SW_RESET: u8 = 0x12;

    /// HV Ready Detection (0x14)
    ///
    /// Parameters:
    /// - A[7:0] = 00h [POR]
    ///
    /// The command requires CLKEN=1 and ANALOGEN=1.
    /// After this command is initiated, HV Ready detection starts.
    /// BUSY pad will output high during detection.
    /// The detection result can be read from the Status Bit Read (Command 0x2F).
    pub const HV_READY: u8 = 0x14;

    /// VCI Detection (0x15)
    ///
    /// Parameters:
    /// - D[0]: 0 = VDHR/VCI detection disable (default), 1 = VDHR/VCI detection enable
    ///
    /// Enable VCI level detection functionality.
    /// After this command is initiated, VCI level detection starts.
    /// BUSY pad will output high during this operation.
    pub const VCI_DETECTION: u8 = 0x15;

    /// Temperature Sensor Control (0x18)
    ///
    /// Parameters:
    /// - A[7:0] = 80h for Internal temperature sensor
    ///
    /// Controls the temperature sensor operation.
    /// Used for temperature compensation in the display.
    pub const TEMP_CONTROL: u8 = 0x18;

    /// Temperature Sensor Control (Write to temperature register) (0x1A)
    ///
    /// Parameters:
    /// - D[7:0]: Temperature value to write
    ///
    /// Writes a value to the internal temperature register.
    pub const TEMP_CONTROL_WRITE: u8 = 0x1A;

    /// Temperature Sensor Control (Read from temperature register) (0x1B)
    ///
    /// Reads the current value from the temperature register.
    /// Returns the measured temperature value.
    pub const TEMP_CONTROL_READ: u8 = 0x1B;

    /// Temperature Sensor Control (Write to external temperature register) (0x1C)
    ///
    /// Parameters:
    /// - D[7:0]: External temperature value to write
    ///
    /// Writes a value to the external temperature register.
    /// Used when an external temperature sensor is employed.
    pub const TEMP_CONTROL_WRITE_EXTERNAL: u8 = 0x1C;

    /// Master Activation (0x20)
    ///
    /// Activates the display update sequence.
    /// The Display Update Sequence Option is configured in register 0x22.
    /// BUSY pad will output high during operation.
    ///
    /// Important: User should not interrupt this operation to avoid
    /// corruption of panel images.
    pub const MASTER_ACTIVATE: u8 = 0x20;

    /// Display Update Control 1 (0x21)
    ///
    /// Parameters:
    /// - D[7:0]: RAM options, defines which RAM to use for display refresh
    ///   - D[0]: Whether to use RAM 0x24 (Black/White)
    ///   - D[1]: Whether to use RAM 0x26 (Red)
    ///   - D[7:2]: Reserved
    /// - Default: 03h (use both RAM 0x24 and 0x26)
    ///
    /// Controls which RAM data is used for display update.
    pub const DISPLAY_UPDATE_CTRL1: u8 = 0x21;

    /// Display Update Control 2 (0x22)
    ///
    /// Parameters:
    /// - D[7:0]: Update sequence options
    ///   - D[7:4]: Display options
    ///     - 0h = Normal, 1h = bypass RAM, ignore RAM content
    ///   - D[3:0]: Sequence options
    ///     - 0h = Clock only, 1h = Clock & analog
    ///     - 2h = Clock, analog, load temp
    ///     - 3h = Clock, analog, load temp, load LUT
    ///     - 4h = All, including display refresh
    /// - Default: C7h
    ///
    /// Configures the update sequence for the display controller.
    pub const DISPLAY_UPDATE_CTRL2: u8 = 0x22;

    /// Write RAM (Black White) / RAM 0x24
    ///
    /// Used to write data to the Black/White RAM.
    /// After this command, data written on the MCU bus will be stored in the B/W RAM.
    /// RAM content determines which pixels are black and which are white.
    pub const WRITE_BW_DATA: u8 = 0x24;

    ///  Write RAM (RED) / RAM 0x26
    ///
    /// Used to write data to the Red RAM.
    /// After this command, data written on the MCU bus will be stored in the Red RAM.
    /// RAM content determines which pixels are red (for tri-color displays).
    pub const WRITE_RED_DATA: u8 = 0x26;

    /// Read RAM (0x27)
    ///
    /// After this command, data read on the MCU bus will fetch data from RAM.
    /// The RAM being read (B/W or Red) is determined by the previous memory access.
    pub const READ_DATA: u8 = 0x27;

    /// VCOM Sense (0x28)
    ///
    /// Parameters:
    /// - D[7:0]: VCOM sense settings
    ///
    /// Initiates the VCOM sense operation.
    /// BUSY pad will output high during this operation.
    pub const VCOM_SENSE: u8 = 0x28;

    /// VCOM Sense Duration (0x29)
    ///
    /// Parameters:
    /// - D[7:0]: Duration value for VCOM sensing
    ///
    /// Sets the duration for the VCOM sense operation.
    pub const VCOM_SENSE_DURATION: u8 = 0x29;

    /// Program VCOM OTP (0x2A)
    ///
    /// Programs the VCOM value into the OTP memory.
    /// This is typically a one-time operation during manufacturing.
    pub const PROGRAM_VCOM_OTP: u8 = 0x2A;

    /// Write Register for VCOM Control (0x2B)
    ///
    /// Parameters:
    /// - D[7:0]: VCOM value
    ///   - VCOM = -0.1V - (VCOM * 0.05V)
    ///   - Default: 28h (-2.0V)
    ///
    /// Controls the VCOM voltage level, which influences display contrast.
    pub const WRITE_VCOM_CONTROL_REGISTER: u8 = 0x2B;

    /// Write VCOM Register (0x2C)
    ///
    /// Parameters:
    /// - D[7:0]: VCOM register value
    ///
    /// Writes a value directly to the VCOM register.
    pub const WRITE_VCOM_REGISTER: u8 = 0x2C;

    /// OTP Register Read for Display Option (0x2D)
    ///
    /// Reads the OTP registers related to display options.
    /// Returns the configuration settings stored in OTP memory.
    pub const OTP_REGISTER_READ: u8 = 0x2D;

    /// User ID Read (0x2E)
    ///
    /// Reads the user ID from the device.
    /// Returns user-programmable identification data.
    pub const USER_ID_READ: u8 = 0x2E;

    /// Status Bit Read (0x2F)
    ///
    /// Returns the status register value with various flags:
    /// - D[0]: BUSY flag (0: Ready, 1: Busy)
    /// - D[1]: Operation in progress (0: No operation in progress, 1: Operation in progress)
    /// - D[2]: Display state (0: Normal, 1: In update)
    /// - D[3]: HV ready status
    /// - D[4]: VCOM sense complete
    /// - D[5]: LUT operation result
    /// - D[6]: Temperature read complete
    /// - D[7]: Power status
    pub const STATUS_BIT_READ: u8 = 0x2F;

    /// Program WS OTP (0x30)
    ///
    /// Parameters:
    /// - D[7:0]: Waveform settings data
    ///
    /// Programs waveform settings into the OTP memory.
    pub const PROGRAM_WS_OTP: u8 = 0x30;

    /// Load WS OTP (0x31)
    ///
    /// Loads the waveform settings from OTP memory.
    /// After this command, the programmed waveform settings will be active.
    pub const LOAD_WS_OTP: u8 = 0x31;

    /// Write LUT Register (0x32)
    ///
    /// Parameters:
    /// - D[n]: LUT (Look-Up Table) data, multiple bytes
    ///
    /// Writes data to the LUT register.
    /// The LUT controls the waveform pattern for display updates.
    /// Different LUTs can be used for different update effects (quality vs. speed).
    pub const WRITE_LUT_REGISTER: u8 = 0x32;

    /// CRC Calculation (0x34)
    ///
    /// Initiates a CRC calculation on the device.
    /// Used to verify data integrity.
    pub const CRC_CALCULATION: u8 = 0x34;

    /// CRC Status Read (0x35)
    ///
    /// Reads the CRC calculation status and result.
    /// Returns the calculated CRC value.
    pub const CRC_STATUS_READ: u8 = 0x35;

    /// Program OTP Selection (0x36)
    ///
    /// Parameters:
    /// - D[7:0]: OTP selection settings
    ///
    /// Selects which OTP memory section to program.
    pub const PROGRAM_OTP_SELECTION: u8 = 0x36;

    /// Write Register for Display Option (0x37)
    ///
    /// Parameters:
    /// - D[7:0]: Display option settings
    ///
    /// Configures various display options and behaviors.
    pub const WRITE_REGISTER_FOR_DISPLAY_OPTION: u8 = 0x37;

    /// Write Register for User ID (0x38)
    ///
    /// Parameters:
    /// - D[n]: User ID data, multiple bytes
    ///
    /// Writes user-defined identification data to the device.
    /// Can be used to store application-specific information.
    pub const WRITE_REGISTER_FOR_USER_ID: u8 = 0x38;

    /// OTP Program Mode (0x39)
    ///
    /// Parameters:
    /// - D[7:0]: OTP program mode settings
    ///
    /// Sets the programming mode for OTP operations.
    pub const OTP_PROGRAM_MODE: u8 = 0x39;

    /// Border Waveform Control (0x3C)
    ///
    /// Parameters:
    /// - D[3:0]: Border waveform color/state
    ///   - 00h = Black border
    ///   - 01h = White border
    ///   - 02h = Red border (for tri-color displays)
    ///   - 03h = Reserved
    ///   - Default: 05h (latch the last frame value)
    /// - D[7:4]: Fixed at 0101b
    ///
    /// Controls the appearance of the display border.
    pub const BORDER_WAVEFORM_CONTROL: u8 = 0x3C;

    /// End Option (EOPT) (0x3F)
    ///
    /// Parameters:
    /// - D[7:0]: End option settings
    ///
    /// Specifies additional options for display updates.
    pub const END_OPTION: u8 = 0x3F;

    /// Read RAM Option (0x41)
    ///
    /// Parameters:
    /// - D[7:0]: RAM read option settings
    ///
    /// Configures how RAM data is read from the device.
    pub const READ_RAM_OPTION: u8 = 0x41;

    /// Set RAM X - Address Start / End Position (0x44)
    ///
    /// Parameters:
    /// - D[7:0]: X-address start position (0 to 295)
    /// - D[15:8]: X-address end position (0 to 295)
    ///
    /// Sets the start and end positions of the X-address in RAM.
    /// These values define the active area width for RAM operations.
    pub const SET_RAMX_START_END: u8 = 0x44;

    /// Set RAM Y - Address Start / End Position (0x45)
    ///
    /// Parameters:
    /// - D[7:0]: Y-address start position (0 to 127)
    /// - D[15:8]: Y-address end position (0 to 127)
    ///
    /// Sets the start and end positions of the Y-address in RAM.
    /// These values define the active area height for RAM operations.
    pub const SET_RAMY_START_END: u8 = 0x45;

    /// Auto Write RED RAM for Regular Pattern (0x46)
    ///
    /// Parameters:
    /// - D[7:0]: Pattern data for Red RAM
    ///
    /// Automatically fills the Red RAM with a regular pattern.
    /// Useful for quickly initializing the Red RAM to a specific pattern.
    pub const AUTO_WRITE_RED_RAM_FOR_REGULAR_PATTERN: u8 = 0x46;

    /// Auto Write B/W RAM for Regular Pattern (0x47)
    ///
    /// Parameters:
    /// - D[7:0]: Pattern data for B/W RAM
    ///
    /// Automatically fills the Black/White RAM with a regular pattern.
    /// Useful for quickly initializing the B/W RAM to a specific pattern.
    pub const AUTO_WRITE_BW_RAM_FOR_REGULAR_PATTERN: u8 = 0x47;

    /// Set RAM X Address Counter (0x4E)
    ///
    /// Parameters:
    /// - D[7:0]: X-address counter value (0 to 295)
    ///
    /// Sets the RAM X-address counter to a specific value.
    /// Subsequent RAM read/write operations will start from this X position.
    pub const SET_RAMX_COUNTER: u8 = 0x4E;

    /// Set RAM Y Address Counter (0x4F)
    ///
    /// Parameters:
    /// - D[7:0]: Y-address counter value (0 to 127)
    ///
    /// Sets the RAM Y-address counter to a specific value.
    /// Subsequent RAM read/write operations will start from this Y position.
    pub const SET_RAMY_COUNTER: u8 = 0x4F;

    /// NOP - No Operation (0xE3)
    ///
    /// This command is an empty command; it does not have any effect on the display module.
    /// However, it can be used to terminate Frame Memory Write or Read Commands.
    /// It can also be used as a dummy command when specific timing requirements need to be met.
    pub const NOP: u8 = 0xE3;
}
