//! Display interface using SPI
use crate::ssd1680::{cmd::Cmd, HEIGHT, WIDTH};
use display_interface::DisplayError;
use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::SpiDevice,
};

const RESET_DELAY_MS: u8 = 200;
const BUSY_WAIT_TIMEOUT_MS: u32 = 5000; // 5 seconds timeout

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
        log::info!("Hardware reset");
        self.reset(delay)?;
        delay.delay_ms(100); // Extra delay after hardware reset
        
        // Software reset - start with a clean state
        log::info!("Software reset");
        self.cmd(Cmd::SW_RESET)?;
        self.wait_busy_low();
        delay.delay_ms(10);

        // Step 1: Driver Output Control
        log::info!("Driver output control");
        self.cmd(Cmd::DRIVER_CONTROL)?;
        self.data(&[0x27])?; // MUX gates = 296 (0x127+1) lines for 2.9" display
        self.data(&[0x01])?; // GS=1 (gate scan from G0 to G(N-1))
        self.data(&[0x00])?; // SM=0, TB=0 (normal color)
        delay.delay_ms(10);

        // Step 2: Set data entry mode - critical for proper pixel mapping
        log::info!("Data entry mode");
        self.cmd(Cmd::DATA_ENTRY_MODE)?;
        self.data(&[0x03])?; // X-increment, Y-increment mode (0x03)
        delay.delay_ms(10);
        
        // Step 3: Set RAM X/Y window to match display size
        log::info!("RAM X/Y window");
        // X window: 0..15 (16 bytes wide = 128 pixels / 8)
        self.cmd(Cmd::SET_RAMX_START_END)?;
        self.data(&[0x00, 0x0F])?;
        delay.delay_ms(10);
        
        // Y window: 0..295
        self.cmd(Cmd::SET_RAMY_START_END)?;
        self.data(&[0x00, 0x00])?; // Y start = 0
        self.data(&[0x27, 0x01])?; // Y end = 0x0127 = 295
        delay.delay_ms(10);
        
        // Step 4: Set RAM X/Y position to start at (0,0)
        log::info!("RAM X/Y position");
        self.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.data(&[0x00])?; // X position = 0
        
        self.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.data(&[0x00, 0x00])?; // Y position = 0
        delay.delay_ms(10);
        
        // Step 5: Set Border Waveform
        log::info!("Border waveform");
        self.cmd(Cmd::BORDER_WAVEFORM_CONTROL)?;
        self.data(&[0x01])?; // White border (0x01)
        delay.delay_ms(10);
        
        // Step 6: Set up temperature sensor
        log::info!("Temperature sensor");
        self.cmd(Cmd::TEMP_CONTROL)?;
        self.data(&[0x80])?; // Internal temperature sensor
        delay.delay_ms(10);
        
        // Step 7: Set booster configuration for better reliability
        log::info!("Booster soft start");
        self.cmd(Cmd::BOOST_SOFT_START_CONTROL)?;
        self.data(&[0x17, 0x17, 0x17])?; // Default values from datasheet
        delay.delay_ms(10);
        
        // Step 8: Set gate/source voltages for display stability
        log::info!("Set gate/source voltages");
        self.cmd(0x03)?; // GATE_VOLTAGE_CONTROL
        self.data(&[0x19])?; // VGH=15V
        delay.delay_ms(10);
        
        self.cmd(0x04)?; // SOURCE_VOLTAGE_CONTROL
        self.data(&[0x02, 0x0C, 0x0C])?;  // VSH/VSL values for good contrast
        delay.delay_ms(10);
        
        // Step 9: Set VCOM value (critical for display quality)
        log::info!("Set VCOM value");
        self.cmd(0x2C)?; // VCOM_REGISTER
        self.data(&[0x28])?; // VCOM = -1.4V
        delay.delay_ms(10);

        // Final wait for any pending operations
        self.wait_busy_low();
        delay.delay_ms(10);

        log::info!("Initialization sequence complete per SSD1680 datasheet");
        Ok(())
    }

    /// Setup RAM windows and pointer for the SSD1680
    fn setup_ram_window(&mut self, _delay: &mut impl DelayNs) -> Result<(), DisplayError> {
        log::info!("Setting up RAM window");
        let ram_x_start: u8 = 0;
        let ram_x_end: u8 = ((WIDTH / 8) - 1) as u8; // 296 / 8 - 1 = 36

        let ram_y_start: u16 = 0;
        let ram_y_end: u16 = HEIGHT - 1; // 175

        // X window: 0..36
        self.cmd(Cmd::SET_RAMX_START_END)?;
        self.data(&[ram_x_start, ram_x_end])?;

        // Y window: 0..175 (little endian)
        self.cmd(Cmd::SET_RAMY_START_END)?;
        self.data(&[
            (ram_y_start & 0xFF) as u8, // Start position LSB
            (ram_y_start >> 8) as u8,   // Start position MSB
            (ram_y_end & 0xFF) as u8,   // End position LSB
            (ram_y_end >> 8) as u8,     // End position MSB
        ])?;

        // Set pointer to top-left corner
        self.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.data(&[ram_x_start])?;

        self.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.data(&[(ram_y_start & 0xFF) as u8, (ram_y_start >> 8) as u8])?;
        Ok(())
    }

    /// Basic function for sending commands
    pub(crate) fn cmd(&mut self, command: u8) -> Result<(), DisplayError> {
        log::info!("Sending command: 0x{:02X}", command);
        
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
        // Only log for small data packets or first packet of a large transfer
        // This prevents log flooding which causes watchdog timeouts
        if data.len() == 1 {
            // For small control bytes, we can log each one
            log::debug!("Data byte: 0x{:02X}", data[0]);
        } else if data.len() <= 4 {
            // For small chunks, log all bytes
            log::debug!("Data: {:?}", data);
        } else {
            // For large chunks, just log the size to avoid flooding logs
            log::debug!("Sending {} bytes of data", data.len());
        }
        
        // high for data
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;
        self.spi
            .write(data)
            .map_err(|_| DisplayError::BusWriteError)
    }

    /// Update the display - exactly matching Arduino EPD_Update function
    pub fn updating(&mut self) -> Result<(), DisplayError> {
        // Match Arduino EPD_Update function exactly
        self.cmd(0x22)?; // DISPLAY_UPDATE_CTRL2
        self.data(&[0xF4])?; // Arduino uses 0xF4 for full refresh

        self.cmd(0x20)?; // MASTER_ACTIVATE

        // Wait until idle after triggering update - exactly like Arduino
        self.wait_busy_low();

        Ok(())
    }

    /// Basic function for sending a command and the data belonging to it.
    pub(crate) fn cmd_with_data(&mut self, command: u8, data: &[u8]) -> Result<(), DisplayError> {
        self.cmd(command)?;
        self.data(data)
    }

    /// Basic function for sending the same byte of data (one u8) multiple times over spi
    /// Used for setting one color for the whole frame
    pub(crate) fn data_x_times(&mut self, val: u8, repetitions: u32) -> Result<(), DisplayError> {
        // Log with limited detail to avoid watchdog timeouts
        log::info!("Sending {} bytes of value 0x{:02X}", repetitions, val);
        
        // high for data
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;
        
        // Use a buffer to send multiple bytes at once for better efficiency
        // This reduces SPI overhead and prevents watchdog timeouts
        const CHUNK_SIZE: usize = 32; // Send 32 bytes at a time
        let buffer = [val; CHUNK_SIZE];
        
        // Calculate full chunks and remaining bytes
        let full_chunks = (repetitions as usize) / CHUNK_SIZE;
        let remainder = (repetitions as usize) % CHUNK_SIZE;
        
        // Log progress periodically during large transfers
        let log_interval = (full_chunks / 10).max(1); // Log ~10 times during transfer
        
        // Send full chunks
        for i in 0..full_chunks {
            // Log progress periodically
            if i % log_interval == 0 && full_chunks > 10 {
                log::debug!("Progress: {}/{} chunks ({:.1}%)", 
                           i, full_chunks, 
                           100.0 * i as f32 / full_chunks as f32);
            }
            
            // Allow watchdog reset between chunks by yielding
            if i > 0 && i % 100 == 0 {
                // Allow other tasks to run and reset watchdog
                std::hint::spin_loop();
            }
            
            self.spi.write(&buffer).map_err(|_| DisplayError::BusWriteError)?;
        }
        
        // Send remaining bytes
        if remainder > 0 {
            self.spi.write(&buffer[0..remainder])
                .map_err(|_| DisplayError::BusWriteError)?;
        }
        
        log::debug!("Completed sending {} bytes of data", repetitions);
        Ok(())
    }

    /// Waits until device isn't busy anymore - just calls wait_busy_low which matches Arduino
    pub(crate) fn wait_until_idle(
        &mut self,
        _delay: &mut impl DelayNs,
    ) -> Result<(), DisplayError> {
        log::info!("Waiting for device to be ready");

        // Just call our wait_busy_low function which matches Arduino's EPD_READBUSY exactly
        self.wait_busy_low();

        Ok(())
    }
    /// Wait for busy pin to go LOW - Arduino EPD_READBUSY implementation with safety timeout
    pub fn wait_busy_low(&mut self) {
        log::info!("EPD_READBUSY: Waiting for busy pin to go LOW...");

        // Similar to Arduino but with a safety timeout to prevent infinite loops
        let max_attempts = 50_000_000; // Very high limit but prevents infinite hang
        let mut counter = 0u32;

        while counter < max_attempts {
            // Check if busy pin is low (not busy)
            match self.busy.is_high() {
                Ok(false) => {
                    // Busy pin is LOW - we're done waiting
                    log::info!(
                        "EPD_READBUSY: BUSY pin is now LOW after {} iterations",
                        counter
                    );
                    return;
                }
                Ok(true) => {
                    // Busy pin is still HIGH - continue waiting
                    counter += 1;
                    if counter % 1_000_000 == 0 {
                        log::info!(
                            "Still waiting for BUSY pin... (count: {}M)",
                            counter / 1_000_000
                        );
                    }
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
        log::info!("Performing reset sequence");

        // Exactly matching Arduino EPD_HW_RESET function
        self.rst.set_high().map_err(|_| DisplayError::RSError)?;
        delay.delay_ms(20);
        self.rst.set_low().map_err(|_| DisplayError::RSError)?;
        delay.delay_ms(2);
        self.rst.set_high().map_err(|_| DisplayError::RSError)?;
        delay.delay_ms(20);

        // Check the busy line after reset (Arduino doesn't do this, but good for diagnostics)
        let busy_state = self.busy.is_high().unwrap_or(true);
        log::info!("Post-reset BUSY pin state: {}", busy_state);

        // Don't wait for idle here - some displays might still show busy
        // after reset until properly initialized

        Ok(())
    }

    /// Read the status register to diagnose busy issues
    pub(crate) fn read_status_register(&mut self) -> Result<u8, DisplayError> {
        log::info!("Reading status register");
        self.cmd(0x2F)?; // READ_STATUS_REGISTER command

        // Set data line to high for reading
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;

        // Read one byte of data
        let mut buffer = [0u8; 1];
        self.spi
            .read(&mut buffer)
            .map_err(|_| DisplayError::BusWriteError)?;

        log::info!("Status register value: 0x{:02X}", buffer[0]);

        // Decode and log the status bits
        if buffer[0] & 0x01 != 0 {
            // STATUS_BUSY_BIT
            log::info!("Status: BUSY (controller is busy)");
        } else {
            log::info!("Status: READY (controller is ready)");
        }

        if buffer[0] & 0x02 != 0 {
            // STATUS_OPERATION_BIT
            log::info!("Status: OPERATION IN PROGRESS");
        }

        if buffer[0] & 0x04 != 0 {
            // STATUS_DISPLAY_BIT
            log::info!("Status: DISPLAY UPDATE IN PROGRESS");
        }

        if buffer[0] & 0x80 != 0 {
            // STATUS_POWER_BIT
            log::info!("Status: POWER ON");
        } else {
            log::info!("Status: POWER OFF");
        }

        Ok(buffer[0])
    }

    /// Set reset pin high directly for hardware testing
    pub fn reset_pin_high(&mut self) -> Result<(), DisplayError> {
        self.rst.set_high().map_err(|_| DisplayError::RSError)
    }

    /// Set reset pin low directly for hardware testing
    pub fn reset_pin_low(&mut self) -> Result<(), DisplayError> {
        self.rst.set_low().map_err(|_| DisplayError::RSError)
    }

    /// Set data/command pin high (data mode) directly for hardware testing
    pub fn dc_pin_high(&mut self) -> Result<(), DisplayError> {
        self.dc.set_high().map_err(|_| DisplayError::DCError)
    }

    /// Set data/command pin low (command mode) directly for hardware testing
    pub fn dc_pin_low(&mut self) -> Result<(), DisplayError> {
        self.dc.set_low().map_err(|_| DisplayError::DCError)
    }

    /// Read busy pin state directly for hardware testing
    pub fn busy_pin_state(&mut self) -> Result<bool, DisplayError> {
        // Since the DisplayError enum doesn't have a BusyError variant, map to DCError
        self.busy.is_high().map_err(|_| DisplayError::DCError)
    }

    /// Read data from the display controller
    pub fn read_data(&mut self, len: usize) -> Result<Vec<u8>, DisplayError> {
        // Set data line high for reading
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;

        // Create a buffer to hold the read data
        let mut buffer = vec![0u8; len];

        // Read the data
        self.spi
            .read(&mut buffer)
            .map_err(|_| DisplayError::BusWriteError)?;

        Ok(buffer)
    }
}
