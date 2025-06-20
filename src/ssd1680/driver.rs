//! Driver for interacting with SSD1680 display driver
pub use display_interface::DisplayError;

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::ssd1680::interface::DisplayInterface;
use crate::ssd1680::{cmd, color, flag, HEIGHT, WIDTH};

/// A configured display with a hardware interface.
pub struct Ssd1680<SPI, BSY, RST, DC> {
    interface: DisplayInterface<SPI, BSY, RST, DC>,
}

impl<SPI, BSY, DC, RST> Ssd1680<SPI, BSY, DC, RST>
where
    SPI: SpiDevice,
    RST: OutputPin,
    DC: OutputPin,
    BSY: InputPin,
{
    /// Create and initialize the display driver
    pub fn new(
        spi: SPI,
        busy: BSY,
        dc: DC,
        rst: RST,
        delay: &mut impl DelayNs,
    ) -> Result<Self, DisplayError>
    where
        Self: Sized,
    {
        let interface = DisplayInterface::new(spi, busy, dc, rst);
        let mut ssd1680 = Ssd1680 { interface };
        ssd1680.init(delay)?;
        Ok(ssd1680)
    }

    /// Initialise the controller
    pub fn init(&mut self, delay: &mut impl DelayNs) -> Result<(), DisplayError> {
        self.interface.init(delay);

        self.use_full_frame()?;

        self.interface.wait_until_idle(delay)?;
        Ok(())
    }

    /// Update the whole buffer on the display driver
    pub fn update_frame(&mut self, buffer: &[u8]) -> Result<(), DisplayError> {
        self.use_full_frame()?;
        self.interface
            .cmd_with_data(cmd::Cmd::WRITE_BW_DATA, buffer)
    }
    /// Wake up the device if it is in sleep mode
    pub fn wake_up(&mut self, delay: &mut impl DelayNs) -> Result<(), DisplayError> {
        log::info!("Waking up the device");
        self.interface
            .cmd_with_data(cmd::Cmd::DEEP_SLEEP_MODE, &[0x00])?;
        self.interface.wait_until_idle(delay)?;
        Ok(())
    }

    /// Start an update of the whole display
    pub fn display_frame(&mut self, delay: &mut impl DelayNs) -> Result<(), DisplayError> {
        log::info!("Sending display update control command");
        self.interface.cmd_with_data(
            cmd::Cmd::DISPLAY_UPDATE_CTRL2,
            &[flag::Flag::DISPLAY_MODE_1],
        )?;
        log::info!("Sending master activate command");
        self.interface.cmd(cmd::Cmd::MASTER_ACTIVATE)?;

        log::info!("Waiting for a short delay after master activate command");
        delay.delay_ms(100);

        log::info!("Waiting until display is idle");
        self.interface.wait_until_idle(delay)?;

        log::info!("Display frame update completed");
        Ok(())
    }

    /// Make the whole black and white frame on the display driver white
    pub fn clear_frame(&mut self) -> Result<(), DisplayError> {
        self.use_full_frame()?;

        // TODO: allow non-white background color
        let color = color::Color::White.get_byte_value();

        self.interface.cmd(cmd::Cmd::WRITE_BW_DATA)?;
        self.interface
            .data_x_times(color, u32::from(WIDTH) / 8 * u32::from(HEIGHT))?;
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
        self.interface.cmd_with_data(
            cmd::Cmd::SET_RAMX_COUNTER, // Set RAM X - address counter
            &[(start_x >> 3) as u8, (end_x >> 3) as u8],
        )?;

        self.interface.cmd_with_data(
            cmd::Cmd::SET_RAMY_COUNTER, // Set RAM Y - address counter
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
            .cmd_with_data(cmd::Cmd::SET_RAMX_COUNTER, &[(x >> 3) as u8])?;

        // 2 Databytes: A[7:0] & 0..A[8]
        self.interface
            .cmd_with_data(cmd::Cmd::SET_RAMY_COUNTER, &[y as u8, (y >> 8) as u8])?;
        Ok(())
    }
}
