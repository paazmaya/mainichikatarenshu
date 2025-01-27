//! Display interface using SPI
use display_interface::DisplayError;
use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
    spi::SpiDevice,
};

const RESET_DELAY_MS: u8 = 10;
const BUSY_WAIT_TIMEOUT_MS: u32 = 5000; // 5 seconds timeout

/// The Connection Interface of all (?) Waveshare EPD-Devices
///
pub(crate) struct DisplayInterface<SPI, BSY, DC, RST> {
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
    /// Basic function for sending commands
    pub(crate) fn cmd(&mut self, command: u8) -> Result<(), DisplayError> {
        log::info!("Sending command: 0x{:02X}", command);
        // low for commands
        self.dc.set_low().map_err(|_| DisplayError::DCError)?;

        // Transfer the command over spi
        self.spi
            .write(&[command])
            .map_err(|_| DisplayError::BusWriteError)
    }

    /// Basic function for sending an array of u8-values of data over spi
    pub(crate) fn data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        log::info!("Sending data.len(): {:?}", data.len());
        // high for data
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;

        // Transfer data (u8-array) over spi
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
        let _ = self.dc.set_high();
        // Transfer data (u8) over spi
        for _ in 0..repetitions {
            self.spi
                .write(&[val])
                .map_err(|_| DisplayError::BusWriteError)?;
        }
        Ok(())
    }

    /// Waits until device isn't busy anymore (busy == HIGH) with a timeout
    pub(crate) fn wait_until_idle(&mut self, delay: &mut impl DelayNs) -> Result<(), DisplayError> {
        log::info!("Entering wait_until_idle");
        let mut elapsed_time = 0;
        while self.busy.is_high().unwrap_or(true) {
            if elapsed_time >= BUSY_WAIT_TIMEOUT_MS {
                log::error!("Timeout waiting for device to become idle");
                return Err(DisplayError::BusWriteError);
            }
            log::info!("Device is busy, waiting...");
            delay.delay_ms(10);
            elapsed_time += 10;
        }
        log::info!("Device is idle");
        Ok(())
    }

    /// Resets the device.
    pub(crate) fn reset(&mut self, delay: &mut impl DelayNs) {
        log::info!("Resetting device");
        self.rst.set_low().unwrap();
        delay.delay_ms(RESET_DELAY_MS.into());
        self.rst.set_high().unwrap();
        delay.delay_ms(RESET_DELAY_MS.into());
        log::info!("Device reset complete, waiting for idle");
        self.wait_until_idle(delay);
    }
}
