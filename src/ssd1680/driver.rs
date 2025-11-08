//! Driver for interacting with SSD1680 display driver
pub use display_interface::DisplayError;

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::ssd1680::interface::DisplayInterface;
use crate::ssd1680::{cmd::Cmd, color, flag::Flag, HEIGHT, WIDTH};

/// A configured display with a hardware interface.
pub struct Ssd1680<SPI, BSY, RST, DC, DELAY> {
    /// The display interface
    pub interface: DisplayInterface<SPI, BSY, RST, DC, DELAY>,
}

impl<SPI, BSY, DC, RST, DELAY> Ssd1680<SPI, BSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    RST: OutputPin,
    DC: OutputPin,
    BSY: InputPin,
    DELAY: DelayNs,
{
    /// Simplified LUT values for SSD1680 from known working implementations
    /// This is a minimal LUT that should work with most SSD1680 displays
    const LUT_FULL_UPDATE: [u8; 70] = [
        // These LUT values are from a known working reference implementation
        0x02, 0x02, 0x01, 0x11, 0x12, 0x12, 0x22, // LUT0: Black
        0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, // LUT1: White
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT2: Red/B
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT3: Red/W
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT4: VCOM
        0x04, 0x14, 0x0A, 0x14, 0x01, // TP0: Phase 0 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP1: Phase 1 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP2: Phase 2 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP3: Phase 3 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP4: Phase 4 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP5: Phase 5 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP6: Phase 6 timing
    ];

    /// Fast update LUT values for SSD1680 (quicker refresh but may have ghosting)
    #[allow(dead_code)]
    const LUT_FAST_UPDATE: [u8; 70] = [
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT0: Black
        0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT1: White
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT2: Red
        0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT3: Red
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT4: VCOM
        0x0A, 0x00, 0x00, 0x00, 0x00, // TP0: Phase 0 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP1: Phase 1 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP2: Phase 2 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP3: Phase 3 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP4: Phase 4 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP5: Phase 5 timing
        0x00, 0x00, 0x00, 0x00, 0x00, // TP6: Phase 6 timing
    ];

    /// Create and initialize the display driver
    pub fn new(spi: SPI, busy: BSY, dc: DC, rst: RST, delay: DELAY) -> Result<Self, DisplayError>
    where
        Self: Sized,
    {
        let interface = DisplayInterface::new(spi, busy, dc, rst, delay);
        let mut ssd1680 = Ssd1680 { interface };
        ssd1680.init()?;
        Ok(ssd1680)
    }

    /// Create a new instance from an existing interface without initialization
    pub fn from_interface(interface: DisplayInterface<SPI, BSY, DC, RST, DELAY>) -> Self
    where
        Self: Sized,
    {
        Ssd1680 { interface }
    }

    /// Set the Look-Up Table (LUT) for the display
    /// This is critical for proper display updates
    fn set_lut(&mut self, lut_data: &[u8]) -> Result<(), DisplayError> {
        log::info!("Setting LUT data");
        self.interface
            .cmd_with_data(Cmd::WRITE_LUT_REGISTER, lut_data)
    }

    /// Initialise the controller with a more robust sequence
    pub fn init(&mut self) -> Result<(), DisplayError> {
        log::info!("Starting full display initialization");

        // Hardware initialization - reset and basic setup
        self.interface.init()?;
        self.interface.delay.delay_ms(50); // Extra delay after hardware init

        // ----------------------
        // Power Configuration
        // ----------------------

        // Booster Soft Start Configuration
        self.interface.cmd_with_data(
            Cmd::BOOST_SOFT_START_CONTROL,
            &[
                Flag::BOOSTER_SOFT_START_PHASE1_DEFAULT,
                Flag::BOOSTER_SOFT_START_PHASE2_DEFAULT,
                Flag::BOOSTER_SOFT_START_PHASE3_DEFAULT,
            ],
        )?;
        self.interface.delay.delay_ms(10);

        // Gate Driving Voltage
        self.interface.cmd_with_data(
            Cmd::GATE_VOLTAGE_CONTROL,
            &[Flag::GATE_VOLTAGE_VGH_DEFAULT], // 15V
        )?;
        self.interface.delay.delay_ms(10);

        // Source Driving Voltage
        self.interface.cmd_with_data(
            Cmd::SOURCE_VOLTAGE_CONTROL,
            &[0x02, 0x0C, 0x0C], // VSH1/VSH2/VSL values
        )?;
        self.interface.delay.delay_ms(10);

        // VCOM Control
        self.interface.cmd_with_data(
            Cmd::WRITE_VCOM_CONTROL_REGISTER,
            &[Flag::VCOM_DEFAULT], // Using moderate VCOM value: approximately -1.4V
        )?;
        self.interface.delay.delay_ms(10);

        // ----------------------
        // RAM Area Configuration
        // ----------------------

        // Configure frame area to ensure proper display operation
        self.use_full_frame()?;
        self.interface.delay.delay_ms(10);

        // ----------------------
        // LUT Configuration
        // ----------------------

        // Set the LUT - critical for proper display operation
        self.set_lut(&Self::LUT_FULL_UPDATE)?;
        self.interface.delay.delay_ms(50); // Give more time for LUT to load properly

        // ----------------------
        // Initial Display Pattern
        // ----------------------

        // Arduino EPD_Init() does NOT trigger a display update at the end
        // The commented-out lines 222-225 in EPD_Init.cpp show this was intentional
        // So we skip the initial clear and update here

        Ok(())
    }

    /// Update the whole buffer on the display driver
    pub fn update_frame(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        self.use_full_frame()?;

        // NOTE: This display appears to have inverted polarity (0xFF=black, 0x00=white)
        // The C++ epd_display_image() inverts data, but that might be for a different display config
        // For now, send data directly without inversion
        // Previously: inverted data with !b
        self.interface.cmd_with_data(Cmd::WRITE_BW_DATA, buffer)?;

        // Always display frame after updating to see changes
        // We'll let the caller handle the delay to avoid blocking here
        Ok(())
    }

    /// Update the whole buffer on the display driver with optional data inversion
    pub fn update_frame_with_inversion(
        &mut self,
        buffer: &[u8],
        invert: bool,
    ) -> Result<(), DisplayError> {
        self.use_full_frame()?;

        if !invert {
            // Use original data
            self.interface.cmd_with_data(Cmd::WRITE_BW_DATA, buffer)?;
        } else {
            // Invert all bytes before sending
            log::info!("Inverting data for display");
            let inverted: Vec<u8> = buffer.iter().map(|&b| !b).collect();
            self.interface
                .cmd_with_data(Cmd::WRITE_BW_DATA, &inverted)?;
        }

        // Always display frame after updating to see changes
        // We'll let the caller handle the delay to avoid blocking here
        Ok(())
    }

    /// Wake up the device if it is in sleep mode
    pub fn wake_up(&mut self) -> Result<(), DisplayError> {
        log::info!("Waking up the device");
        self.interface
            .cmd_with_data(Cmd::DEEP_SLEEP_MODE, &[Flag::DEEP_SLEEP_NORMAL_MODE])?;
        self.interface.wait_busy_low();
        Ok(())
    }

    /// Display frame with all required steps to update the display
    pub fn display_frame(&mut self) -> Result<(), DisplayError> {
        log::info!("Starting display frame update process with improved sequence");

        // Before starting a new update, make sure the display is idle
        // Some displays need this check to avoid conflicts
        self.interface.wait_busy_low();

        // Different display update sequence based on SSD1680 datasheet

        // Skip power on command as it's not defined in our command set
        // Instead just add a substantial delay before starting update sequence
        self.interface.delay.delay_ms(100); // Substantial delay before update sequence

        // 1. Set display update control 1
        self.interface.cmd_with_data(
            Cmd::DISPLAY_UPDATE_CTRL1,
            &[Flag::DISPLAY_UPDATE_BW_RAM], // Update only B/W RAM for simpler operation
        )?;
        self.interface.delay.delay_ms(20); // Longer delay to ensure command is processed

        // 2. Set display update control 2
        self.interface.cmd_with_data(
            Cmd::DISPLAY_UPDATE_CTRL2,
            &[Flag::DISPLAY_UPDATE_FULL], // Value from working C++ implementation
        )?;
        self.interface.delay.delay_ms(20); // Longer delay to ensure command is processed

        // 3. Master activate - this actually triggers the display update
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        // Critical delay right after Master Activate - this is when the display begins updating
        self.interface.delay.delay_ms(300); // Longer delay to ensure update starts properly

        // 4. Wait for display to be ready
        self.interface.wait_busy_low();

        // 5. Send NOP to terminate the update sequence
        self.interface.cmd(Cmd::NOP)?;

        // Extra stabilization delay after display update completes
        self.interface.delay.delay_ms(200); // Longer stability delay

        log::info!("Display frame update completed successfully");
        Ok(())
    }

    /// Make the whole black and white frame on the display driver white
    pub fn clear_frame(&mut self) -> Result<(), DisplayError> {
        log::info!("Clearing frame to white");
        self.use_full_frame()?;

        // Clear frame with white
        let color = color::Color::White.get_byte_value();

        // Write white data
        self.interface.cmd(Cmd::WRITE_BW_DATA)?;

        let total_bytes = u32::from(WIDTH) / 8 * u32::from(HEIGHT);

        self.interface.data_x_times(color, total_bytes)?;

        // Update display
        self.display_frame()?;

        Ok(())
    }

    /// Draw a test pattern on the display to verify it's working properly
    /// Uses a more reliable pattern that's easier to see on e-paper displays
    pub fn draw_test_pattern(&mut self) -> Result<(), DisplayError> {
        log::info!("Drawing high-contrast test pattern");
        self.use_full_frame()?;

        // Calculate buffer size
        let width_bytes = (WIDTH as u32).div_ceil(8); // Round up to nearest byte
        let height = HEIGHT as u32;
        let buffer_size = (width_bytes * height) as usize;

        // *** IMPORTANT: Try both black and white backgrounds to see what works ***
        // Some displays invert the colors from what we expect
        let invert_colors = false; // Try changing to true if display shows inverted pattern

        // Create buffer - start with either all black or all white
        let mut buffer = vec![if invert_colors { 0xFF } else { 0x00 }; buffer_size];

        // 1. Draw large diagonal stripes - very visible pattern
        let stripe_width = 16; // Wide stripes (16 pixels)

        for y in 0..height {
            for x_byte in 0..width_bytes {
                for bit in 0..8 {
                    let x = x_byte * 8 + bit;
                    if x >= WIDTH as u32 {
                        continue;
                    }

                    // Create diagonal pattern with stripes
                    let diagonal_pos = (x + y) / stripe_width;
                    let is_stripe = diagonal_pos % 2 == 0;

                    // Calculate bit position within the byte
                    let bit_mask = 0x80 >> bit;

                    if is_stripe {
                        // Set or clear the bit based on stripe pattern
                        if invert_colors {
                            buffer[(y * width_bytes + x_byte) as usize] &= !bit_mask;
                        } else {
                            buffer[(y * width_bytes + x_byte) as usize] |= bit_mask;
                        }
                    }
                }
            }
        }

        // 2. Add a thick border - very visible
        const BORDER_WIDTH: u32 = 8; // Extra thick border

        // Top and bottom borders
        for y in 0..BORDER_WIDTH {
            for x_byte in 0..width_bytes {
                let top_idx = (y * width_bytes + x_byte) as usize;
                if buffer_size > top_idx {
                    buffer[top_idx] = if invert_colors { 0x00 } else { 0xFF };
                }

                let bottom_idx = ((height - 1 - y) * width_bytes + x_byte) as usize;
                if buffer_size > bottom_idx {
                    buffer[bottom_idx] = if invert_colors { 0x00 } else { 0xFF };
                }
            }
        }

        // Left and right borders
        for y in BORDER_WIDTH..(height - BORDER_WIDTH) {
            for x_byte in 0..BORDER_WIDTH.min(width_bytes) {
                let left_idx = (y * width_bytes + x_byte) as usize;
                if buffer_size > left_idx {
                    buffer[left_idx] = if invert_colors { 0x00 } else { 0xFF };
                }

                let right_idx = (y * width_bytes + (width_bytes - 1 - x_byte)) as usize;
                if buffer_size > right_idx && right_idx >= (y * width_bytes) as usize {
                    buffer[right_idx] = if invert_colors { 0x00 } else { 0xFF };
                }
            }
        }

        // 3. Draw a large X pattern across the display - very visible
        for y in 0..height {
            // Calculate the byte and bit for the two diagonal lines
            let x1 = (y * WIDTH as u32) / height;
            let x2 = WIDTH as u32 - 1 - x1;

            // Make the lines thicker (3 pixels wide)
            for thickness in -1..=1 {
                let x1t = x1.saturating_add_signed(thickness);
                let x2t = x2.saturating_add_signed(thickness);

                let byte1 = (x1t / 8) as usize;
                let bit1 = 7 - (x1t % 8) as usize;

                let byte2 = (x2t / 8) as usize;
                let bit2 = 7 - (x2t % 8) as usize;

                if byte1 < width_bytes as usize {
                    let idx = (y * width_bytes + byte1 as u32) as usize;
                    if idx < buffer_size {
                        if invert_colors {
                            buffer[idx] &= !(1 << bit1);
                        } else {
                            buffer[idx] |= 1 << bit1;
                        }
                    }
                }

                if byte2 < width_bytes as usize {
                    let idx = (y * width_bytes + byte2 as u32) as usize;
                    if idx < buffer_size {
                        if invert_colors {
                            buffer[idx] &= !(1 << bit2);
                        } else {
                            buffer[idx] |= 1 << bit2;
                        }
                    }
                }
            }
        }

        // Add large blocks of solid black and white in opposite corners
        log::info!("Adding solid blocks in corners");
        let block_size = 24;

        // Top-right: solid block
        for y in BORDER_WIDTH..(BORDER_WIDTH + block_size) {
            for x_byte in (width_bytes - 3)..(width_bytes - BORDER_WIDTH.min(width_bytes)) {
                let idx = (y * width_bytes + x_byte) as usize;
                if idx < buffer_size {
                    buffer[idx] = if invert_colors { 0x00 } else { 0xFF };
                }
            }
        }

        // Bottom-left: solid block
        for y in (height - BORDER_WIDTH - block_size)..(height - BORDER_WIDTH) {
            for x_byte in BORDER_WIDTH..(BORDER_WIDTH + 3) {
                let idx = (y * width_bytes + x_byte) as usize;
                if idx < buffer_size {
                    buffer[idx] = if invert_colors { 0x00 } else { 0xFF };
                }
            }
        }

        // Try with and without data inversion to see which works
        let try_inversion = true;

        // First attempt without inversion
        log::info!("Test pattern - first attempt (normal data)");
        log::info!("Writing test pattern to display memory");
        self.update_frame_with_inversion(&buffer, false)?;
        log::info!("Updating display with test pattern");
        self.display_frame()?;
        self.interface.delay.delay_ms(1000);

        if try_inversion {
            // Second attempt with inversion
            log::info!("Test pattern - second attempt (inverted data)");
            log::info!("Writing inverted test pattern to display memory");
            self.update_frame_with_inversion(&buffer, true)?;
            log::info!("Updating display with inverted test pattern");
            self.display_frame()?;
            self.interface.delay.delay_ms(1000);
        }

        log::info!("Test pattern drawn successfully");
        Ok(())
    }

    /// Draws a very simple test pattern - half screen black, half screen white
    /// This creates the most basic pattern possible to verify display function
    pub fn draw_simple_test_pattern(&mut self) -> Result<(), DisplayError> {
        log::info!("Drawing simple split-screen test pattern (half black, half white)");

        // Ensure display is ready before starting
        self.interface.wait_busy_low();

        // Setup the RAM window to cover the whole display
        self.use_full_frame()?;

        // Calculate buffer size
        let width_bytes = (WIDTH as u32).div_ceil(8);
        let height = HEIGHT as u32;
        let buffer_size = (width_bytes * height) as usize;

        // Create a buffer where the top half is black (0x00) and bottom half is white (0xFF)
        let mut buffer = vec![0x00; buffer_size];

        // Fill the bottom half with white
        let half_height = height / 2;
        for y in half_height..height {
            for x_byte in 0..width_bytes {
                let idx = (y * width_bytes + x_byte) as usize;
                if idx < buffer_size {
                    buffer[idx] = 0xFF;
                }
            }
        }

        // Try first with normal data
        log::info!("Updating with simple pattern (normal data)");
        self.update_frame(&buffer)?;
        self.display_frame()?;
        self.interface.delay.delay_ms(1000);

        // Then try with inverted data
        log::info!("Updating with simple pattern (inverted data)");
        let inverted: Vec<u8> = buffer.iter().map(|&b| !b).collect();
        self.update_frame(&inverted)?;
        self.display_frame()?;

        log::info!("Simple test pattern display complete");
        Ok(())
    }

    /// Create an ultra-basic test pattern of solid white and solid black areas
    /// This provides the maximum contrast possible to verify the display is working
    pub fn white_and_black_test_pattern(&mut self) -> Result<(), DisplayError> {
        log::info!("Drawing ultra-basic white and black test pattern");

        // 1. Hardware reset first to ensure clean state
        log::info!("Performing hardware reset");
        self.interface.reset()?;
        self.interface.delay.delay_ms(200);

        // 2. Configure display for full frame update
        log::info!("Setting up for full frame update");
        self.use_full_frame()?;

        // Calculate buffer size
        let width_bytes = WIDTH as usize / 8;
        let height = HEIGHT as usize;
        let buffer_size = width_bytes * height;

        log::info!("Creating buffer of size {} bytes", buffer_size);

        // Create buffer with these regions:
        // - Top half: completely white (0xFF)
        // - Bottom half: completely black (0x00)
        let mut buffer = vec![0xFF; buffer_size]; // Start with all white

        // Fill bottom half with black
        let half_height = height / 2;
        log::info!(
            "Setting bottom half to black (rows {}-{})",
            half_height,
            height - 1
        );

        for y in half_height..height {
            for x in 0..width_bytes {
                let index = y * width_bytes + x;
                buffer[index] = 0x00; // Black
            }
        }

        // 3. Write the buffer to RAM
        log::info!("Writing test pattern to RAM");
        self.update_frame(&buffer)?;

        // 4. Update the display with the test pattern
        log::info!("Updating display with test pattern");
        self.display_frame()?;

        // 5. Wait for a while to let the pattern display
        log::info!("Test pattern should now be visible");
        self.interface.delay.delay_ms(1000);

        // 6. Try inverting the pattern and displaying again
        log::info!("Now trying inverted pattern");

        // Invert the buffer (white becomes black, black becomes white)
        for i in 0..buffer_size {
            buffer[i] = !buffer[i];
        }

        // Write inverted buffer and update display
        log::info!("Writing inverted test pattern to RAM");
        self.update_frame(&buffer)?;

        log::info!("Updating display with inverted test pattern");
        self.display_frame()?;

        log::info!("Basic test pattern display completed");
        Ok(())
    }

    /// Put device into deep sleep mode to save power
    pub fn sleep(&mut self) -> Result<(), DisplayError> {
        log::info!("Putting display into deep sleep mode");
        self.interface
            .cmd_with_data(Cmd::DEEP_SLEEP_MODE, &[0x01])?;
        log::info!("Display now in deep sleep mode");
        Ok(())
    }

    /// Fast update sequence matching the working C++ implementation (epd_fast_update)
    /// This provides quicker refreshes but may have more ghosting than full updates
    pub fn fast_update(&mut self) -> Result<(), DisplayError> {
        log::info!("Starting fast update sequence from C++ implementation");

        // Hardware reset as per C++ implementation
        self.interface.reset()?;
        self.interface.delay.delay_ms(100);

        // First update sequence: 0xB1 - start reading temperature, load LUT, full update, clock off
        log::info!("Fast update step 1: Temperature read and LUT load");
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL2, &[0xB1])?;
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        self.interface.wait_busy_low();

        // Write temperature parameter
        log::info!("Fast update step 2: Write temperature parameter");
        self.interface
            .cmd_with_data(Cmd::TEMP_CONTROL_WRITE, &[0x64, 0x00])?;

        // Second update sequence: 0x91
        log::info!("Fast update step 3: Update with 0x91");
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL2, &[0x91])?;
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        self.interface.wait_busy_low();

        // Third update sequence: 0xC7
        log::info!("Fast update step 4: Final update with 0xC7");
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL2, &[0xC7])?;
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        self.interface.wait_busy_low();

        log::info!("Fast update sequence completed");
        Ok(())
    }

    /// Emergency clear function - alternative approach to clear display to white
    /// This tries a different initialization and update sequence compared to factory_reset_clear
    pub fn emergency_clear(&mut self) -> Result<(), DisplayError> {
        log::info!("EMERGENCY: Attempting alternative clear operation");

        // Step 1: Extended reset sequence
        log::info!("Extended hardware reset");
        self.interface.reset()?;
        self.interface.delay.delay_ms(500); // Very long reset time

        // Step 2: Basic initialization with minimal commands
        log::info!("Basic initialization sequence");

        // Software reset
        self.interface.cmd(Cmd::SW_RESET)?;
        self.interface.delay.delay_ms(200);

        // Driver output control - 296 gate lines (critical for 2.9")
        log::info!("Setting driver control");
        // Try original values first
        self.interface
            .cmd_with_data(Cmd::DRIVER_CONTROL, &[0x27, 0x01, 0x00])?; // 296 lines, normal scan
        self.interface.delay.delay_ms(100);

        // Data entry mode
        log::info!("Setting data entry mode");
        self.interface
            .cmd_with_data(Cmd::DATA_ENTRY_MODE, &[0x03])?; // Y+, X+
        self.interface.delay.delay_ms(100);

        // Step 3: Try display update and RAM clear without LUT settings
        // This is the most minimal approach possible

        // Set RAM window to cover entire display
        log::info!("Setting RAM window to full display size");

        // X range: 0 to (WIDTH/8 - 1)
        self.interface.cmd(Cmd::SET_RAMX_START_END)?;
        self.interface.data(&[0x00, ((WIDTH / 8) - 1) as u8])?;
        self.interface.delay.delay_ms(50);

        // Y range: 0 to (HEIGHT - 1)
        self.interface.cmd(Cmd::SET_RAMY_START_END)?;
        // Y address is little-endian in this driver
        let y_end = HEIGHT - 1;
        self.interface.data(&[
            0x00,
            0x00,                        // Y start: 0
            (y_end & 0xFF) as u8,        // Y end LSB (e.g., 0x27 for 295)
            ((y_end >> 8) & 0xFF) as u8, // Y end MSB (e.g., 0x01 for 295)
        ])?;
        self.interface.delay.delay_ms(50);

        // Set RAM address counter to (0,0)
        log::info!("Setting RAM counter to origin (0,0)");
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;
        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?; // Little-endian: Y=0
        self.interface.delay.delay_ms(50);

        // Step 4: Try direct pattern writing approach first
        log::info!("APPROACH 1: Using auto-write pattern for white screen");

        // Use the auto-write pattern command for faster filling
        self.interface.cmd_with_data(
            Cmd::AUTO_WRITE_BW_RAM_FOR_REGULAR_PATTERN,
            &[0xFF], // 0xFF = all white
        )?;
        self.interface.delay.delay_ms(200);

        // Step 5: Alternative approach with manual RAM writing
        log::info!("APPROACH 2: Manual RAM fill with white data");

        // Reset RAM pointers
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;
        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?;
        self.interface.delay.delay_ms(50);

        // Write white data to RAM
        self.interface.cmd(Cmd::WRITE_BW_DATA)?;

        // Calculate total bytes needed for the full frame
        let width_bytes = WIDTH as u32 / 8;
        let total_bytes = width_bytes * HEIGHT as u32;

        // Fill in smaller chunks with progress reporting
        let chunk_size = 64; // Smaller chunks for more reliable transmission
        log::info!("Writing {} bytes of WHITE data in chunks", total_bytes);

        for i in 0..total_bytes.div_ceil(chunk_size) {
            let bytes_remaining = total_bytes - i * chunk_size;
            let bytes_to_write = bytes_remaining.min(chunk_size);

            if i % 10 == 0 {
                log::info!(
                    "Writing chunk {}/{}",
                    i + 1,
                    total_bytes.div_ceil(chunk_size)
                );
            }

            if bytes_to_write > 0 {
                self.interface.data_x_times(0xFF, bytes_to_write)?;
                self.interface.delay.delay_ms(5); // Small inter-chunk delay
            }
        }

        // Step 6: Try both update methods for redundancy

        // Method 1: Standard update sequence
        log::info!("UPDATE METHOD 1: Standard update sequence");
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL1, &[0x01])?; // B/W RAM only
        self.interface.delay.delay_ms(50);
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL2, &[0xC7])?; // Standard value
        self.interface.delay.delay_ms(50);
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        // Longer delay to give display time to update
        log::info!("Waiting for first update to complete");
        self.interface.delay.delay_ms(500);
        self.interface.wait_busy_low();
        self.interface.delay.delay_ms(200); // Additional delay after reaching idle

        // Method 2: Alternative update sequence
        log::info!("UPDATE METHOD 2: Fast update sequence");
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL2, &[0xF7])?; // Different value
        self.interface.delay.delay_ms(50);
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        // Long delay for display to update
        log::info!("Waiting for second update to complete");
        self.interface.delay.delay_ms(500);
        self.interface.wait_busy_low();

        // Final stability delay
        self.interface.delay.delay_ms(300);

        log::info!("Emergency alternative clear completed");
        Ok(())
    }

    /// Last resort: Factory-reset-style full display clear to white
    /// This is a complete standalone sequence that tries multiple approaches to clear the display
    pub fn factory_reset_clear(&mut self) -> Result<(), DisplayError> {
        log::info!("EMERGENCY: Performing comprehensive factory-reset-style clear sequence");

        // Step 1: Multiple hardware resets to ensure clean start
        log::info!("Performing multiple hardware resets");
        for i in 0..3 {
            log::info!("Hardware reset cycle {}", i + 1);
            self.interface.reset()?;
            self.interface.delay.delay_ms(300);
        }

        // Step 2: Force gate/source drivers to known good state
        log::info!("Configuring power settings");

        // Software reset
        log::info!("Software reset");
        self.interface.cmd(Cmd::SW_RESET)?;
        self.interface.delay.delay_ms(300);

        // Booster soft start control - from manufacturer datasheet
        log::info!("Setting booster soft start");
        self.interface
            .cmd_with_data(Cmd::BOOST_SOFT_START_CONTROL, &[0xD7, 0xD6, 0x9D])?;
        self.interface.delay.delay_ms(50);

        // Power settings
        log::info!("Setting gate voltage");
        self.interface
            .cmd_with_data(Cmd::GATE_VOLTAGE_CONTROL, &[0x19])?;
        self.interface.delay.delay_ms(50);

        log::info!("Setting source voltage");
        self.interface
            .cmd_with_data(Cmd::SOURCE_VOLTAGE_CONTROL, &[0x02, 0x0C, 0x0C])?;
        self.interface.delay.delay_ms(50);

        log::info!("Setting VCOM voltage");
        self.interface
            .cmd_with_data(Cmd::WRITE_VCOM_REGISTER, &[0xA8])?;
        self.interface.delay.delay_ms(50);

        // Step 3: Driver output control - correctly specify display dimensions
        log::info!("Setting driver output control for 2.9-inch display (296 gate lines)");
        self.interface
            .cmd_with_data(Cmd::DRIVER_CONTROL, &[0x27, 0x01, 0x00])?;
        self.interface.delay.delay_ms(50);

        // Step 4: Data entry mode - ensure proper RAM address increments
        log::info!("Setting data entry mode to Y+, X+");
        self.interface
            .cmd_with_data(Cmd::DATA_ENTRY_MODE, &[0x03])?; // Y+, X+
        self.interface.delay.delay_ms(50);

        // Step 5: Configure RAM window using HEIGHT and WIDTH constants
        log::info!("Setting RAM window");

        // X window: 0 to (WIDTH/8 - 1)
        let ram_x_end = ((WIDTH / 8) - 1) as u8;
        log::info!("Setting X window: 0 to {} for {} pixels", ram_x_end, WIDTH);
        self.interface.cmd(Cmd::SET_RAMX_START_END)?;
        self.interface.data(&[0x00, ram_x_end])?;
        self.interface.delay.delay_ms(50);

        // Y window: 0 to (HEIGHT - 1)
        log::info!(
            "Setting Y window: 0 to {} for {} pixels",
            HEIGHT - 1,
            HEIGHT
        );
        self.interface.cmd(Cmd::SET_RAMY_START_END)?;
        // Remember Y address is little-endian (LSB first)
        self.interface.data(&[
            (0 & 0xFF) as u8,            // Y start LSB
            (0 >> 8) as u8,              // Y start MSB
            ((HEIGHT - 1) & 0xFF) as u8, // Y end LSB
            ((HEIGHT - 1) >> 8) as u8,   // Y end MSB
        ])?;
        self.interface.delay.delay_ms(50);

        // Step 6: Set RAM counters to start position
        log::info!("Setting RAM counters to origin (0,0)");
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;
        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?; // Y=0 (LSB first)
        self.interface.delay.delay_ms(50);

        // Step 7: Set up LUT for white clear
        log::info!("Setting minimal LUT for clearing to white");
        // Simple LUT with only what's needed to clear to white
        let clear_lut: [u8; 30] = [
            // Only use phase 0 with simple values
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT1: White
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Timing
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Unused
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Unused
            0x00, 0x00, // Padding
        ];
        self.interface
            .cmd_with_data(Cmd::WRITE_LUT_REGISTER, &clear_lut)?;
        self.interface.delay.delay_ms(100);

        // Step 8: Write white data to ALL RAM, trying both normal and auto-write methods

        // Method 1: Normal RAM write
        log::info!("CLEARING METHOD 1: Normal RAM write with white data");
        self.interface.cmd(Cmd::WRITE_BW_DATA)?;

        let total_bytes = (WIDTH as u32 / 8) * (HEIGHT as u32);
        log::info!("Writing {} bytes of WHITE data (0xFF)", total_bytes);

        // Write all white data in smaller chunks
        let chunk_size = 64;
        for i in 0..total_bytes.div_ceil(chunk_size) {
            let remaining = total_bytes - i * chunk_size;
            let bytes_to_write = remaining.min(chunk_size);

            if bytes_to_write > 0 {
                self.interface.data_x_times(0xFF, bytes_to_write)?;
                self.interface.delay.delay_ms(2); // Small delay between chunks
            }
        }
        self.interface.delay.delay_ms(100);

        // Method 2: Auto write pattern
        log::info!("CLEARING METHOD 2: Auto write pattern with white");
        self.interface.cmd_with_data(
            Cmd::AUTO_WRITE_BW_RAM_FOR_REGULAR_PATTERN,
            &[0xFF], // All white pattern
        )?;
        self.interface.delay.delay_ms(100);

        // Step 9: Border waveform control - set white border
        log::info!("Setting white border");
        self.interface
            .cmd_with_data(Cmd::BORDER_WAVEFORM_CONTROL, &[0x51])?; // White border (0x50|0x01)
        self.interface.delay.delay_ms(50);

        // Step 10: Configure and execute display update
        log::info!("Performing display update");

        // First attempt with more aggressive update settings
        log::info!("Update method 1: Full update sequence");
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL1, &[0x01])?; // B/W RAM only
        self.interface.delay.delay_ms(50);
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL2, &[0xC7])?; // Standard value
        self.interface.delay.delay_ms(50);
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        // Long wait for display to update
        log::info!("Waiting for display update to complete");
        self.interface.delay.delay_ms(500);
        self.interface.wait_busy_low();

        // Second attempt with different update settings in case the first didn't work
        log::info!("Update method 2: Alternative update sequence");
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL1, &[0x01])?;
        self.interface.delay.delay_ms(50);
        self.interface
            .cmd_with_data(Cmd::DISPLAY_UPDATE_CTRL2, &[0xF7])?; // Different value
        self.interface.delay.delay_ms(50);
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        // Another long wait
        log::info!("Waiting for second update to complete");
        self.interface.delay.delay_ms(500);
        self.interface.wait_busy_low();

        // Final stability delay
        self.interface.delay.delay_ms(300);

        log::info!("Comprehensive factory reset clear sequence completed");
        Ok(())
    }

    /// Special initialization sequence for 2.9" SSD1680 e-paper displays
    /// This is customized for this specific display based on manufacturer documentation
    /// Revised to match exact specifications for 2.9-inch displays
    pub fn init_2point9_inch(&mut self) -> Result<(), DisplayError> {
        log::info!("Starting REVISED initialization for 2.9-inch SSD1680 display");

        // Double hardware reset for better reliability
        log::info!("Hardware reset sequence (first reset)");
        self.interface.reset()?;
        self.interface.delay.delay_ms(200); // Longer delay

        log::info!("Hardware reset sequence (second reset)");
        self.interface.reset()?;
        self.interface.delay.delay_ms(200);

        // Software reset
        log::info!("Software reset");
        self.interface.cmd(Cmd::SW_RESET)?;
        self.interface.delay.delay_ms(200); // Extended delay after reset

        // Device should be responding after reset
        log::info!("Device reset complete");

        // Driver output control - CRITICAL SETTING FOR 2.9" DISPLAY WITH 296 GATE LINES
        log::info!("Setting driver control for 2.9-inch (296 gate lines)");
        // 0x27=39, 0x01=1, 0x00=0 => 296 lines (0x127=295), scan direction, output polarity
        self.interface
            .cmd_with_data(Cmd::DRIVER_CONTROL, &[0x27, 0x01, 0x00])?;
        self.interface.delay.delay_ms(20);

        // Booster soft start - CRITICAL FOR PROPER POWER SEQUENCE
        log::info!("Setting booster soft start parameters");
        // These values are from confirmed working 2.9" e-paper displays
        self.interface
            .cmd_with_data(Cmd::BOOST_SOFT_START_CONTROL, &[0xD7, 0xD6, 0x9D])?;
        self.interface.delay.delay_ms(20);

        // Write VCOM register - Important for contrast
        log::info!("Setting VCOM register for proper contrast");
        self.interface
            .cmd_with_data(Cmd::WRITE_VCOM_REGISTER, &[0xA8])?;
        self.interface.delay.delay_ms(20);

        // Set dummy line period
        log::info!("Setting dummy line period");
        self.interface.cmd_with_data(0x3A, &[0x1A])?;
        self.interface.delay.delay_ms(20);

        // Set gate time
        log::info!("Setting gate time");
        self.interface.cmd_with_data(0x3B, &[0x08])?;
        self.interface.delay.delay_ms(20);

        // Set data entry mode - CRITICAL FOR RAM ADDRESSING DIRECTION
        log::info!("Setting data entry mode (Y increment, X increment)");
        self.interface
            .cmd_with_data(Cmd::DATA_ENTRY_MODE, &[0x03])?; // Y+, X+
        self.interface.delay.delay_ms(20);

        // RAM area configuration - CRITICAL FOR ADDRESSING THE CORRECT DISPLAY AREA

        // Calculate RAM window parameters for 2.9" display
        // WIDTH = 128 pixels, which is 128/8 = 16 bytes per row
        // So X address should be 0x00 to 0x0F (0 to 15)
        // HEIGHT = 296 pixels, so Y address should be 0 to 295 (0x0000 to 0x0127)
        log::info!("Setting RAM X window (0-15) for 128 pixel width");
        self.interface.cmd(Cmd::SET_RAMX_START_END)?;
        self.interface.data(&[0x00, 0x0F])?; // X: 0 to 15 (16 bytes per row for 128 pixels)
        self.interface.delay.delay_ms(20);

        log::info!("Setting RAM Y window (0-295) for 296 pixel height");
        self.interface.cmd(Cmd::SET_RAMY_START_END)?;
        // Y: 0 to 295 (0x0000 to 0x0127)
        self.interface.data(&[0x00, 0x00, 0x27, 0x01])?; // LSB first: 0x0127 = 295
        self.interface.delay.delay_ms(20);

        // Set RAM counters to starting position (0,0)
        log::info!("Setting RAM counters to origin (0,0)");
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;
        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?; // Y=0 (LSB first)
        self.interface.delay.delay_ms(20);

        // Set border waveform
        log::info!("Setting border waveform to white");
        // 0x01=White border with fixed high bits (0x50 | 0x01 = 0x51)
        self.interface
            .cmd_with_data(Cmd::BORDER_WAVEFORM_CONTROL, &[0x51])?;
        self.interface.delay.delay_ms(20);

        // Set analog block control
        log::info!("Setting analog block control");
        self.interface.cmd_with_data(0x74, &[0x54])?;
        self.interface.delay.delay_ms(20);

        // Set digital block control
        log::info!("Setting digital block control");
        self.interface.cmd_with_data(0x7E, &[0x3B])?;
        self.interface.delay.delay_ms(20);

        // Temperature sensor - use internal sensor
        log::info!("Setting temperature sensor to internal");
        self.interface.cmd_with_data(Cmd::TEMP_CONTROL, &[0x80])?;
        self.interface.delay.delay_ms(20);

        // Load a simplified LUT for reliable operation
        log::info!("Loading simplified LUT for white clear operation");
        self.set_minimal_lut()?;

        // Wait for everything to stabilize
        log::info!("Waiting for stabilization");
        self.interface.wait_busy_low();

        log::info!("Revised 2.9-inch init sequence completed successfully");
        Ok(())
    }

    /// Set a minimal LUT (Look-Up Table) designed only for clearing the screen to white
    /// This simplifies the waveform to improve reliability
    fn set_minimal_lut(&mut self) -> Result<(), DisplayError> {
        log::info!("Setting minimal LUT for basic white operation");

        // Very simple LUT focused only on getting a white screen
        // Most values are zeroed out except essential ones
        let minimal_clear_lut: [u8; 70] = [
            // LUT0: Basic black-to-white transition (phase 0)
            0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // LUT1: Basic white-to-white transition (phase 0)
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT2: Unused
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT3: Unused
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LUT4: VCOM
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Phase timing settings - simplified to just what's needed
            0x0F, 0x00, 0x00, 0x00, 0x00, // Phase 0 timing - longer duration
            0x00, 0x00, 0x00, 0x00, 0x00, // Phase 1
            0x00, 0x00, 0x00, 0x00, 0x00, // Phase 2
            0x00, 0x00, 0x00, 0x00, 0x00, // Phase 3
            0x00, 0x00, 0x00, 0x00, 0x00, // Phase 4
            0x00, 0x00, 0x00, 0x00, 0x00, // Phase 5
            0x00, 0x00, 0x00, 0x00, 0x00, // Phase 6
        ];

        self.interface
            .cmd_with_data(Cmd::WRITE_LUT_REGISTER, &minimal_clear_lut)?;
        self.interface.delay.delay_ms(50); // Give time for LUT to be processed

        log::info!("Minimal LUT set successfully");
        Ok(())
    }

    /// Direct command access - matches Arduino exactly
    pub fn direct_cmd(&mut self, command: u8) -> Result<(), DisplayError> {
        self.interface.cmd(command)
    }

    /// Direct data access - matches Arduino exactly
    pub fn direct_data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        self.interface.data(data)
    }

    /// Wait for BUSY pin to go LOW - matches Arduino EPD_READBUSY
    pub fn wait_busy(&mut self) {
        self.interface.wait_busy_low()
    }

    /// Perform hardware reset - expose interface reset for testing
    pub fn interface_reset(&mut self) -> Result<(), DisplayError> {
        self.interface.reset()
    }

    /// Exact C++ epd_all_fill() implementation for testing
    pub fn cpp_all_fill(&mut self, color: u8) -> Result<(), DisplayError> {
        log::info!("C++ epd_all_fill with color: 0x{:02X}", color);

        // Border waveform control - EXACTLY as in C++
        self.interface.cmd(Cmd::BORDER_WAVEFORM_CONTROL)?;
        if color != 0 {
            self.interface.data(&[Flag::BORDER_WAVEFORM_WHITE])?;
        } else {
            self.interface.data(&[Flag::BORDER_WAVEFORM_BLACK])?;
        }

        // Write RAM (WRITE_BW_DATA)
        self.interface.cmd(Cmd::WRITE_BW_DATA)?;

        // Write color to all bytes
        let total_bytes = (WIDTH as u32 / 8) * HEIGHT as u32;
        self.interface.data_x_times(color, total_bytes)?;

        // Wait for busy - EXACTLY as in C++
        self.interface.wait_busy_low();

        log::info!("C++ epd_all_fill complete");
        Ok(())
    }

    /// Exact C++ epd_update() implementation for testing
    pub fn cpp_update(&mut self) -> Result<(), DisplayError> {
        log::info!("C++ epd_update");

        // DISPLAY_UPDATE_CTRL2 with full update
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[Flag::DISPLAY_UPDATE_FULL])?;

        // MASTER_ACTIVATE
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        // Wait for busy
        self.interface.wait_busy_low();

        log::info!("C++ epd_update complete");
        Ok(())
    }

    /// Exact C++ epd_clear_r26h() implementation
    pub fn cpp_clear_r26h(&mut self) -> Result<(), DisplayError> {
        log::info!("C++ epd_clear_r26h");

        // Write to RED RAM
        self.interface.cmd(Cmd::WRITE_RED_DATA)?;

        // Fill with WHITE (0xFF in C++ code)
        let total_bytes = (WIDTH as u32 / 8) * HEIGHT as u32;
        self.interface.data_x_times(0xFF, total_bytes)?;

        // Wait for busy
        self.interface.wait_busy_low();

        log::info!("C++ epd_clear_r26h complete");
        Ok(())
    }

    /// Exact Arduino EPD_Init() - minimal initialization matching Arduino exactly
    pub fn cpp_init(&mut self) -> Result<(), DisplayError> {
        log::info!("C++ EPD_Init() - exact Arduino initialization");

        // Hardware reset
        self.interface.reset()?;

        // Software reset
        self.interface.cmd(0x12)?;
        self.interface.wait_busy_low();

        // Driver output control
        self.interface.cmd(0x01)?;
        self.interface.data(&[0x27, 0x01, 0x00])?;

        // Data entry mode
        self.interface.cmd(0x11)?;
        self.interface.data(&[0x03])?;

        // RAM X address
        self.interface.cmd(0x44)?;
        self.interface.data(&[0x00, 0x0F])?;

        // RAM Y address
        self.interface.cmd(0x45)?;
        self.interface.data(&[0x00, 0x00, 0x27, 0x01])?;

        // Border waveform
        self.interface.cmd(0x3C)?;
        self.interface.data(&[0x01])?;

        // Temperature sensor
        self.interface.cmd(0x18)?;
        self.interface.data(&[0x80])?;

        // RAM X counter
        self.interface.cmd(0x4E)?;
        self.interface.data(&[0x00])?;

        // RAM Y counter
        self.interface.cmd(0x4F)?;
        self.interface.data(&[0x00, 0x00])?;

        // Final busy wait
        self.interface.wait_busy_low();

        log::info!("C++ EPD_Init() complete");
        Ok(())
    }

    /// Direct update display - trying different refresh mode to fix black screen
    pub fn direct_update_display(&mut self) -> Result<(), DisplayError> {
        // Simplify approach based on working Arduino examples for this specific SSD1680 display
        log::info!("Starting simplified direct display update (Arduino-compatible)");

        // Step 1: Reset RAM X/Y pointers
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;

        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?;

        // Step 2: Enable display update with only BW RAM (most important for basic operation)
        log::info!("Setting Display Update Control 1 (0x21)");
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL1)?;
        self.interface.data(&[0x01])?; // Enable ONLY B/W RAM update, disable red RAM

        // Step 3: Set update control with working C++ value
        log::info!("Setting Display Update Control 2 (0x22) with working C++ value");
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[0xF4])?; // Use working C++ value (was 0xC7 in datasheet)

        // Step 4: Activate
        log::info!("Activating display update with Master Activate (0x20)");
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        // Step 5: Wait until operation completes
        log::info!("Waiting for busy signal to clear...");
        self.interface.wait_busy_low();

        // Note: If the above doesn't work, we'll try the Arduino-specific approach
        log::info!("Arduino-compatible approach completed, display should update now");

        Ok(())
    }

    /// Direct Clear - specialized method that only clears the display to white
    /// This method is the most direct approach to clearing the display to white, bypassing
    /// all other functionality and focusing solely on setting the display to all white pixels.
    pub fn direct_clear(&mut self) -> Result<(), DisplayError> {
        log::info!("DIRECT CLEAR: Starting ultra-direct approach to clear display to white");

        // Step 1: Multiple hardware resets to ensure clean start
        log::info!("Performing multiple hardware resets");
        for _ in 0..2 {
            self.interface.reset()?;
            self.interface.delay.delay_ms(200);
        }

        // Step 2: Software reset to ensure clean state
        log::info!("Software reset");
        self.interface.cmd(Cmd::SW_RESET)?;
        self.interface.delay.delay_ms(200);

        // Step 3: Configure only the essential registers for this display

        // Driver output control - 296 gate lines for 2.9" display (0x127+1 = 296)
        log::info!("Setting driver output control for 296 lines");
        self.interface.cmd(Cmd::DRIVER_CONTROL)?;
        self.interface.data(&[0x27, 0x01, 0x00])?; // 296 lines, normal scan direction
        self.interface.delay.delay_ms(20);

        // Data entry mode - set to Y+, X+ for proper addressing
        log::info!("Setting data entry mode (Y+, X+)");
        self.interface.cmd(Cmd::DATA_ENTRY_MODE)?;
        self.interface.data(&[Flag::DATA_ENTRY_INCRY_INCRX])?;
        self.interface.delay.delay_ms(20);

        // Step 4: Set RAM window to cover the entire display
        log::info!("Setting RAM window to full display size");

        // X address: 0 to (WIDTH/8 - 1) = 0 to 15 for 128 pixel width
        self.interface.cmd(Cmd::SET_RAMX_START_END)?;
        self.interface.data(&[0x00, ((WIDTH / 8) - 1) as u8])?;
        self.interface.delay.delay_ms(20);

        // Y address: 0 to (HEIGHT - 1) = 0 to 295 for 296 pixel height
        // Remember Y address is little-endian (LSB first)
        self.interface.cmd(Cmd::SET_RAMY_START_END)?;
        self.interface.data(&[
            0x00,
            0x00,                      // Y start = 0 (LSB, MSB)
            ((HEIGHT - 1) as u8),      // Y end LSB (295 & 0xFF = 0x27)
            ((HEIGHT - 1) >> 8) as u8, // Y end MSB (295 >> 8 = 0x01)
        ])?;
        self.interface.delay.delay_ms(20);

        // Step 5: Set RAM address counter to (0,0) starting position
        log::info!("Setting RAM address counter to (0,0)");
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;
        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?;
        self.interface.delay.delay_ms(20);

        // Step 6: Fill the entire RAM with white pixels (0xFF)
        log::info!("Writing ALL WHITE (0xFF) to display RAM");

        // Method 1: Use auto write pattern for faster filling
        log::info!("METHOD 1: Using auto write pattern (0xFF)");
        self.interface
            .cmd(Cmd::AUTO_WRITE_BW_RAM_FOR_REGULAR_PATTERN)?;
        self.interface.data(&[0xFF])?; // 0xFF = all white
        self.interface.delay.delay_ms(100);

        // Method 2: Direct RAM write as backup
        log::info!("METHOD 2: Direct RAM write with all white pixels");

        // Reset RAM address counter to (0,0) again
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;
        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?;
        self.interface.delay.delay_ms(20);

        // Write white data
        self.interface.cmd(Cmd::WRITE_BW_DATA)?;

        // Calculate total bytes and write in small chunks
        let total_bytes = u32::from(WIDTH) / 8 * u32::from(HEIGHT);
        log::info!("Writing {} bytes of WHITE (0xFF) data", total_bytes);

        let chunk_size = 64;
        for i in 0..total_bytes.div_ceil(chunk_size) {
            let remaining = total_bytes - i * chunk_size;
            let bytes_to_write = remaining.min(chunk_size);

            if bytes_to_write > 0 {
                if i % 10 == 0 {
                    log::info!(
                        "Writing chunk {}/{}",
                        i + 1,
                        total_bytes.div_ceil(chunk_size)
                    );
                }
                self.interface.data_x_times(0xFF, bytes_to_write)?;
            }
        }
        self.interface.delay.delay_ms(50);

        // Step 7: Configure border to white
        log::info!("Setting border to white");
        self.interface.cmd(Cmd::BORDER_WAVEFORM_CONTROL)?;
        self.interface
            .data(&[Flag::BORDER_WAVEFORM_FIXED_BITS | Flag::BORDER_WAVEFORM_WHITE])?;
        self.interface.delay.delay_ms(20);

        // Step 8: Display update sequence
        log::info!("Starting display update sequence");

        // Display update control 1 - use B/W RAM only
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL1)?;
        self.interface.data(&[Flag::DISPLAY_UPDATE_BW_RAM])?;
        self.interface.delay.delay_ms(20);

        // Display update control 2 - full update
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[Flag::DISPLAY_UPDATE_FULL])?;
        self.interface.delay.delay_ms(20);

        // Master activation - start update
        log::info!("Activating display update");
        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        // Step 9: Wait for update to complete with long timeout
        log::info!("Waiting for display update to complete (this may take a few seconds)");
        self.interface.delay.delay_ms(500); // Pre-wait delay

        // Check busy status
        self.interface.wait_busy_low();

        log::info!("Direct clear operation completed, display should now be white");
        Ok(())
    }

    fn use_full_frame(&mut self) -> Result<(), DisplayError> {
        /*
        Write Image and Drive Display Panel
        • Write image data in RAM by Command 0x4E, 0x4F, 0x24, 0x26
        • Set softstart setting by Command 0x0C
        • Drive display panel by Command 0x22, 0x20
        • Wait BUSY Low
        */
        // choose full frame/ram
        self.set_ram_area(0, 0, u32::from(WIDTH) - 1, u32::from(HEIGHT) - 1)?;

        // start from the beginning
        self.set_ram_counter(0, 0)
    }

    fn set_ram_area(
        &mut self,
        start_x: u32,
        start_y: u32,
        end_x: u32,
        end_y: u32,
    ) -> Result<(), DisplayError> {
        assert!(start_x < end_x);
        assert!(start_y < end_y);

        /*
        Write Image and Drive Display Panel
        • Write image data in RAM by Command 0x4E, 0x4F, 0x24, 0x26
        • Set softstart setting by Command 0x0C
        • Drive display panel by Command 0x22, 0x20
        • Wait BUSY Low
        */
        // Correctly set the X address window
        self.interface.cmd_with_data(
            Cmd::SET_RAMX_START_END, // Set RAM X address start/end position
            &[(start_x >> 3) as u8, (end_x >> 3) as u8],
        )?;

        // Correctly set the Y address window
        self.interface.cmd_with_data(
            Cmd::SET_RAMY_START_END, // Set RAM Y address start/end position
            &[
                start_y as u8,
                (start_y >> 8) as u8,
                end_y as u8,
                (end_y >> 8) as u8,
            ],
        )?;
        Ok(())
    }

    fn set_ram_counter(&mut self, x: u32, y: u32) -> Result<(), DisplayError> {
        // x is positioned in bytes, so the last 3 bits which show the position inside a byte in the ram
        // aren't relevant
        self.interface
            .cmd_with_data(Cmd::SET_RAMX_COUNTER, &[(x >> 3) as u8])?;

        // 2 Databytes: A[7:0] & 0..A[8]
        self.interface
            .cmd_with_data(Cmd::SET_RAMY_COUNTER, &[y as u8, (y >> 8) as u8])?;
        Ok(())
    }

    /// Bare minimum test pattern - tries several approaches one after another
    pub fn bare_minimum_test(&mut self) -> Result<(), DisplayError> {
        log::info!("======================================================");
        log::info!("BARE MINIMUM TEST PATTERN SEQUENCE");
        log::info!("======================================================");

        // Reset the device
        log::info!("Performing hardware reset");
        self.interface.reset()?;
        self.interface.delay.delay_ms(200);

        // Software reset
        log::info!("Performing software reset");
        self.interface.cmd(Cmd::SW_RESET)?;
        self.interface.delay.delay_ms(200);

        // PATTERN 1: Direct RAM writing with solid white
        log::info!("\nPATTERN 1: Solid white (All 0xFF)");

        // Configure basic RAM window
        log::info!("Setting RAM window");
        self.interface.cmd(Cmd::SET_RAMX_START_END)?;
        self.interface.data(&[0x00, 0x0F])?; // 0-15 for 128 pixels

        self.interface.cmd(Cmd::SET_RAMY_START_END)?;
        self.interface.data(&[0x00, 0x00, 0x27, 0x01])?; // 0-295 for 296 pixels

        // Set RAM counters
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;

        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?;

        // Fill RAM with white
        log::info!("Writing solid white to RAM");
        self.interface.cmd(Cmd::WRITE_BW_DATA)?;

        // Fill with white (faster approach)
        self.interface
            .data_x_times(0xFF, u32::from(WIDTH) / 8 * u32::from(HEIGHT))?;

        // Update display
        log::info!("Updating display with white pattern");
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL1)?;
        self.interface.data(&[0x01])?; // B/W RAM only

        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[0xF7])?; // Display update value

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        log::info!("Waiting for update to complete");
        self.interface.delay.delay_ms(1000);
        self.interface.wait_busy_low();

        log::info!("White pattern should now be visible");
        self.interface.delay.delay_ms(3000);

        // PATTERN 2: Direct RAM writing with solid black
        log::info!("\nPATTERN 2: Solid black (All 0x00)");

        // Reset RAM counters
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;

        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?;

        // Fill RAM with black
        log::info!("Writing solid black to RAM");
        self.interface.cmd(Cmd::WRITE_BW_DATA)?;

        // Fill with black (faster approach)
        self.interface
            .data_x_times(0x00, u32::from(WIDTH) / 8 * u32::from(HEIGHT))?;

        // Update display
        log::info!("Updating display with black pattern");
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL1)?;
        self.interface.data(&[0x01])?; // B/W RAM only

        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[0xF7])?; // Display update value

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        log::info!("Waiting for update to complete");
        self.interface.delay.delay_ms(1000);
        self.interface.wait_busy_low();

        log::info!("Black pattern should now be visible");
        self.interface.delay.delay_ms(3000);

        // PATTERN 3: Checkerboard pattern
        log::info!("\nPATTERN 3: Checkerboard pattern");

        // Reset RAM counters
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;

        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?;

        // Fill RAM with checkerboard
        log::info!("Writing checkerboard pattern to RAM");
        self.interface.cmd(Cmd::WRITE_BW_DATA)?;

        // Use alternating patterns
        let total_bytes = u32::from(WIDTH) / 8 * u32::from(HEIGHT);
        for i in 0..total_bytes {
            if i % 2 == 0 {
                self.interface.data(&[0xAA])?; // 10101010
            } else {
                self.interface.data(&[0x55])?; // 01010101
            }
        }

        // Update display
        log::info!("Updating display with checkerboard pattern");
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL1)?;
        self.interface.data(&[0x01])?; // B/W RAM only

        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[0xF7])?; // Display update value

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        log::info!("Waiting for update to complete");
        self.interface.delay.delay_ms(1000);
        self.interface.wait_busy_low();

        log::info!("Checkerboard pattern should now be visible");
        self.interface.delay.delay_ms(3000);

        // PATTERN 4: Try auto pattern fill
        log::info!("\nPATTERN 4: Auto pattern fill (using 0x46 command)");

        // Use auto write pattern command
        log::info!("Using auto pattern write command");
        self.interface
            .cmd(Cmd::AUTO_WRITE_BW_RAM_FOR_REGULAR_PATTERN)?;
        self.interface.data(&[0xFF])?; // Pattern data (all white)

        self.interface.delay.delay_ms(100);

        // Update display
        log::info!("Updating display with auto pattern");
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL1)?;
        self.interface.data(&[0x01])?; // B/W RAM only

        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[0xF7])?; // Display update value

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;

        log::info!("Waiting for update to complete");
        self.interface.delay.delay_ms(1000);
        self.interface.wait_busy_low();

        log::info!("Auto pattern should now be visible");

        log::info!("Bare minimum test sequence completed");
        Ok(())
    }

    /// Wait for busy pin to go LOW - using Arduino approach with safety timeout
    fn arduino_wait_until_idle(&mut self) {
        log::info!("Waiting for BUSY pin to go LOW (Arduino style)");

        // Use the interface's wait_busy_low which now has proper timeout handling
        self.interface.wait_busy_low();
    }

    /// Update the display with full refresh - EXACTLY matching Arduino's EPD_Update sequence
    pub fn arduino_full_update(&mut self) -> Result<(), DisplayError> {
        log::info!("Performing Arduino-style full update (exact EPD_Update implementation)");

        // Display Update Control 2 + Master Activation - matches EPD_Update exactly
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[Flag::DISPLAY_UPDATE_FULL])?; // Arduino uses 0xF4 for full refresh

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        // Use interface's wait_busy_low to match Arduino's EPD_READBUSY
        self.interface.wait_busy_low();

        log::info!("Arduino-compatible full update complete");
        Ok(())
    }

    /// Update the display with fast refresh using Arduino approach
    pub fn arduino_fast_update(&mut self) -> Result<(), DisplayError> {
        log::info!("Performing Arduino-style fast update");

        // Reset first
        self.interface.reset()?;

        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[Flag::DISPLAY_UPDATE_FAST])?; // Arduino uses 0xB1 for fast refresh

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        self.arduino_wait_until_idle();

        // Set temperature parameter
        self.interface.cmd(Cmd::TEMP_CONTROL_WRITE)?; // Write temperature parameter
        self.interface.data(&[0x64, 0x00])?;

        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[Flag::DISPLAY_UPDATE_PARTIAL_1])?; // Arduino uses 0x91

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        self.arduino_wait_until_idle();

        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[Flag::DISPLAY_UPDATE_PARTIAL_2])?; // Arduino uses 0xC7

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        self.arduino_wait_until_idle();

        log::info!("Arduino-compatible fast update complete");
        Ok(())
    }

    /// Display image using Arduino approach - based on EPD_Bitmap function but with improved stability
    pub fn arduino_display_image(&mut self, image_data: &[u8]) -> Result<(), DisplayError> {
        log::info!(
            "Displaying image using Arduino-compatible approach (with stability improvements)"
        );

        // First reset the X and Y address counters to 0
        log::info!("Setting RAM X address to 0");
        self.interface.cmd(Cmd::SET_RAMX_COUNTER)?;
        self.interface.data(&[0x00])?;

        log::info!("Setting RAM Y address to 0");
        self.interface.cmd(Cmd::SET_RAMY_COUNTER)?;
        self.interface.data(&[0x00, 0x00])?;

        // Set border
        log::info!("Setting border to white");
        self.interface.cmd(Cmd::BORDER_WAVEFORM_CONTROL)?;
        self.interface.data(&[Flag::BORDER_WAVEFORM_WHITE])?; // White border

        // Write to RAM
        log::info!("Writing image data to RAM (with inversion)");
        self.interface.cmd(Cmd::WRITE_BW_DATA)?; // Write to RAM (register 24h)

        // Write data in smaller chunks to avoid overwhelming the SPI driver
        // and to provide progress updates
        const CHUNK_SIZE: usize = 128; // Process 128 bytes at a time
        let total_chunks = image_data.len().div_ceil(CHUNK_SIZE);

        for (chunk_idx, chunk) in image_data.chunks(CHUNK_SIZE).enumerate() {
            // Log progress every 8 chunks
            if chunk_idx % 8 == 0 {
                log::info!(
                    "Writing chunk {}/{} ({:.1}%)",
                    chunk_idx + 1,
                    total_chunks,
                    100.0 * (chunk_idx + 1) as f32 / total_chunks as f32
                );
            }

            // Process each byte in the chunk - TRY WITHOUT INVERSION
            for &byte in chunk {
                // Try WITHOUT inverting to fix all-black display issue
                self.interface.data(&[byte])?; // Send without inverting
            }
        }

        // Update display
        self.interface.cmd(Cmd::DISPLAY_UPDATE_CTRL2)?;
        self.interface.data(&[Flag::DISPLAY_UPDATE_FULL])?;

        // Add a small delay between commands
        for _ in 0..1000 {
            core::hint::spin_loop();
        }

        self.interface.cmd(Cmd::MASTER_ACTIVATE)?;
        self.interface.wait_busy_low();

        Ok(())
    }
}
