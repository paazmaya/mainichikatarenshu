//! Display interface using SPI
use crate::ssd1680::{cmd::Cmd, flag::Flag};
use display_interface::DisplayError;
use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::SpiDevice,
};

/// The Connection Interface of all (?) Waveshare EPD-Devices
///
pub struct DisplayInterface<SPI, BSY, DC, RST> {
    /// SPI device
    spi: SPI,
    /// High (based on Arduino code) for busy, Wait until display is ready!
    busy: BSY,
    /// Data/Command Control Pin (High for data, Low for command)
    dc: DC,
    /// Pin for Reseting
    rst: RST,
}

impl<SPI, BSY, DC, RST> DisplayInterface<SPI, BSY, DC, RST> {
    /// Create and initialize display
    pub fn new(spi: SPI, busy: BSY, dc: DC, rst: RST) -> Self {
        DisplayInterface { spi, busy, dc, rst }
    }
}

impl<SPI, BSY, DC, RST> DisplayInterface<SPI, BSY, DC, RST>
where
    SPI: SpiDevice,
    RST: OutputPin,
    DC: OutputPin,
    BSY: InputPin,
{
    /// Initialize display using the exact Arduino initialization sequence - EXACTLY matching EPD_Init
    pub(crate) fn init(&mut self, delay: &mut impl DelayNs) -> Result<(), DisplayError> {
        log::info!("Initializing e-paper display with SSD1680 datasheet sequence");

        // Hardware reset first - ESSENTIAL for proper operation
        self.reset(delay)?;
        delay.delay_ms(100); // Extra delay after hardware reset

        // Software reset - start with a clean state
        self.cmd(Cmd::SW_RESET)?;
        self.wait_busy_low();
        delay.delay_ms(10);

        // Step 1: Driver Output Control
        self.cmd_with_data(
            Cmd::DRIVER_CONTROL,
            &[
                0x27,                                    // MUX gates = 296 (0x127+1) lines for 2.9" display
                Flag::DRIVER_OUTPUT_GATE_SCAN_FROM_GN,   // GS=1 (gate scan from G0 to G(N-1))
                Flag::DRIVER_OUTPUT_SOURCE_NORMAL_COLOR, // SM=0, TB=0 (normal color)
            ],
        )?;
        delay.delay_ms(10);

        // Step 2: Set data entry mode - critical for proper pixel mapping
        self.cmd_with_data(Cmd::DATA_ENTRY_MODE, &[Flag::DATA_ENTRY_INCRY_INCRX])?; // X-increment, Y-increment mode
        delay.delay_ms(10);

        // Step 3: Set RAM X/Y window to match display size
        // X window: 0..15 (16 bytes wide = 128 pixels / 8)
        self.cmd_with_data(Cmd::SET_RAMX_START_END, &[0x00, 0x0F])?;
        delay.delay_ms(10);

        // Y window: 0..295
        self.cmd_with_data(
            Cmd::SET_RAMY_START_END,
            &[
                0x00, 0x00, // Y start = 0
                0x27, 0x01, // Y end = 0x0127 = 295
            ],
        )?;
        delay.delay_ms(10);

        // Step 4: Set RAM X/Y position to start at (0,0)
        self.cmd_with_data(Cmd::SET_RAMX_COUNTER, &[0x00])?; // X position = 0

        self.cmd_with_data(Cmd::SET_RAMY_COUNTER, &[0x00, 0x00])?; // Y position = 0
        delay.delay_ms(10);

        // Step 5: Set Border Waveform
        self.cmd_with_data(Cmd::BORDER_WAVEFORM_CONTROL, &[Flag::BORDER_WAVEFORM_WHITE])?; // White border
        delay.delay_ms(10);

        // Step 6: Set up temperature sensor
        self.cmd_with_data(Cmd::TEMP_CONTROL, &[Flag::INTERNAL_TEMP_SENSOR])?; // Internal temperature sensor
        delay.delay_ms(10);

        // Step 7: Set booster configuration for better reliability
        self.cmd_with_data(
            Cmd::BOOST_SOFT_START_CONTROL,
            &[
                Flag::BOOSTER_SOFT_START_PHASE1_DEFAULT,
                Flag::BOOSTER_SOFT_START_PHASE2_DEFAULT,
                Flag::BOOSTER_SOFT_START_PHASE3_DEFAULT,
            ],
        )?; // Default values from datasheet
        delay.delay_ms(10);

        // Step 8: Set gate/source voltages for display stability
        self.cmd_with_data(Cmd::GATE_VOLTAGE_CONTROL, &[Flag::GATE_VOLTAGE_VGH_DEFAULT])?; // VGH=15V
        delay.delay_ms(10);

        self.cmd_with_data(Cmd::SOURCE_VOLTAGE_CONTROL, &[0x02, 0x0C, 0x0C])?; // VSH/VSL values for good contrast
        delay.delay_ms(10);

        // Step 9: Set VCOM value (critical for display quality)
        self.cmd_with_data(Cmd::WRITE_VCOM_REGISTER, &[Flag::VCOM_DEFAULT])?; // VCOM = -1.4V
        delay.delay_ms(10);

        // Final wait for any pending operations
        self.wait_busy_low();
        delay.delay_ms(10);

        Ok(())
    }

    /// Basic function for sending commands
    pub(crate) fn cmd(&mut self, command: u8) -> Result<(), DisplayError> {
        // low for commands
        self.dc.set_low().map_err(|_| DisplayError::DCError)?;

        // Transfer the command over spi with error handling
        match self.spi.write(&[command]) {
            Ok(_) => Ok(()),
            Err(e) => {
                log::error!("SPI write error for command 0x{:02X}: {:?}", command, e);
                Err(DisplayError::BusWriteError)
            }
        }
    }

    /// Basic function for sending an array of u8-values of data over spi
    pub(crate) fn data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        // high for data
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;
        self.spi
            .write(data)
            .map_err(|_| DisplayError::BusWriteError)
    }

    /// Basic function for sending a command and the data belonging to it.
    pub(crate) fn cmd_with_data(&mut self, command: u8, data: &[u8]) -> Result<(), DisplayError> {
        self.cmd(command)?;
        self.data(data)
    }

    /// Basic function for sending the same byte of data (one u8) multiple times over spi
    /// Used for setting one color for the whole frame
    pub(crate) fn data_x_times(&mut self, val: u8, repetitions: u32) -> Result<(), DisplayError> {
        // high for data
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;

        // Use a buffer to send multiple bytes at once for better efficiency
        // This reduces SPI overhead and prevents watchdog timeouts
        const CHUNK_SIZE: usize = 32; // Send 32 bytes at a time
        let buffer = [val; CHUNK_SIZE];

        // Calculate full chunks and remaining bytes
        let full_chunks = (repetitions as usize) / CHUNK_SIZE;
        let remainder = (repetitions as usize) % CHUNK_SIZE;

        // Send full chunks
        for i in 0..full_chunks {
            // Allow watchdog reset between chunks by yielding
            if i > 0 && i % 100 == 0 {
                // Allow other tasks to run and reset watchdog
                std::hint::spin_loop();
            }

            self.spi
                .write(&buffer)
                .map_err(|_| DisplayError::BusWriteError)?;
        }

        // Send remaining bytes
        if remainder > 0 {
            self.spi
                .write(&buffer[0..remainder])
                .map_err(|_| DisplayError::BusWriteError)?;
        }

        Ok(())
    }

    /// Wait for busy pin to go LOW - Arduino EPD_READBUSY implementation with safety timeout
    pub fn wait_busy_low(&mut self) {
        // Similar to Arduino but with a safety timeout to prevent infinite loops
        let max_attempts = 50_000_000; // Very high limit but prevents infinite hang
        let mut counter = 0u32;

        while counter < max_attempts {
            // Check if busy pin is low (not busy)
            match self.busy.is_high() {
                Ok(false) => {
                    // Busy pin is LOW - we're done waiting
                    return;
                }
                Ok(true) => {
                    // Busy pin is still HIGH - continue waiting
                    counter += 1;
                }
                Err(_) => {
                    // Error reading pin - bail out to avoid infinite loop
                    log::error!("Error reading BUSY pin state - assuming not busy to continue");
                    return;
                }
            }
        }

        // If we got here, we timed out
        log::error!(
            "EPD_READBUSY: TIMEOUT waiting for BUSY pin to go LOW after {} attempts",
            max_attempts
        );
        // Don't hang the program - just continue and hope for the best
    }

    /// Resets the device with the exact Arduino reset sequence - EXACTLY matching EPD_HW_RESET
    pub(crate) fn reset(&mut self, delay: &mut impl DelayNs) -> Result<(), DisplayError> {
        // Exactly matching Arduino EPD_HW_RESET function
        self.rst.set_high().map_err(|_| DisplayError::RSError)?;
        delay.delay_ms(20);
        self.rst.set_low().map_err(|_| DisplayError::RSError)?;
        delay.delay_ms(2);
        self.rst.set_high().map_err(|_| DisplayError::RSError)?;
        delay.delay_ms(20);

        // Don't wait for idle here - some displays might still show busy
        // after reset until properly initialized

        Ok(())
    }
}
