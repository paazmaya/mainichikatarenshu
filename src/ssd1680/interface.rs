//! Display interface using SPI
use crate::ssd1680::{cmd, flag};
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
    /// Initialize the display
    pub(crate) fn init(&mut self, delay: &mut impl DelayNs) {
        /*
        Set Initial Configuration
        • Define SPI interface to communicate with MCU
        • HW Reset
        • SW Reset by Command 0x12
        • Wait 10ms
        */
        self.reset(delay);
        self.cmd(cmd::Cmd::SW_RESET).expect("Failed to send SW_RESET command"); // SWRESET
        delay.delay_ms(10);

        /*
        Send Initialization Code
        • Set gate driver output by Command 0x01
        • Set display RAM size by Command 0x11, 0x44, 0x45
        • Set panel border by Command 0x3C
        */
        self.cmd(cmd::Cmd::DRIVER_CONTROL).expect("Failed to send DRIVER_CONTROL command"); // Driver output control
        self.data(&[0x11, 0x44, 0x45]).expect("Failed to send data for DRIVER_CONTROL command"); // 0x11, 0x44, 0x45
        self.cmd(cmd::Cmd::BORDER_WAVEFORM_CONTROL).expect("Failed to send BORDER_WAVEFORM_CONTROL command"); // Panel border control
        /*
        Load Waveform LUT
        • Sense temperature by int/ext TS by Command 0x18
        • Load waveform LUT from OTP by Command 0x22, 0x20 or by MCU
        • Wait BUSY Low
        */
        self.cmd(cmd::Cmd::TEMP_CONTROL).expect("Failed to send TEMP_CONTROL command"); // Read built-in temperature sensor
        self.data(&[flag::Flag::INTERNAL_TEMP_SENSOR]).expect("Failed to send data for TEMP_CONTROL command"); // 0x80
        
        self.wait_until_idle(delay).expect("Failed to wait until idle");





        self.cmd(cmd::Cmd::DATA_ENTRY_MODE); // Data entry mode
        self.data(&[0x03]); // Was in ssd1680 &[flag::Flag::DATA_ENTRY_INCRY_INCRX]


        self.cmd(cmd::Cmd::BORDER_WAVEFORM_CONTROL); // BorderWaveform
        self.data(&[0x01]); // Was in ssd1680 &[flag::Flag::BORDER_WAVEFORM_FOLLOW_LUT | flag::Flag::BORDER_WAVEFORM_LUT1]
    }

    /// Basic function for sending commands
    pub(crate) fn cmd(&mut self, command: u8) -> Result<(), DisplayError> {
        log::info!("Sending command: 0x{:02X}", command);
        // low for commands
        self.dc.set_low().map_err(|_| DisplayError::DCError)?;

        //self.cs.set_low().map_err(|_| DisplayError::BusWriteError)?;

        // Transfer the command over spi
        

        //self.cs.set_high().map_err(|_| DisplayError::BusWriteError)?;

        self
            .spi
            .write(&[command])
            .map_err(|_| DisplayError::BusWriteError)
    }

    /// Basic function for sending an array of u8-values of data over spi
    pub(crate) fn data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        log::info!("Sending data.len(): {:?}", data.len());
        // high for data
        self.dc.set_high().map_err(|_| DisplayError::DCError)?;

        //self.cs.set_low().map_err(|_| DisplayError::BusWriteError)?;

        // Transfer data (u8-array) over spi
        

        //self.cs.set_high().map_err(|_| DisplayError::BusWriteError)?;

        self
            .spi
            .write(data)
            .map_err(|_| DisplayError::BusWriteError)
    }

    pub fn updating(&mut self) {
        self.cmd(cmd::Cmd::DISPLAY_UPDATE_CTRL2);
        self.data(&[flag::Flag::DISPLAY_UPDATE_FULL]);
        self.cmd(cmd::Cmd::MASTER_ACTIVATE);
        //self.wait_until_idle();
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

    /// Waits until device isn't busy anymore (busy == LOW) with a timeout
    pub(crate) fn wait_until_idle(&mut self, delay: &mut impl DelayNs) -> Result<(), DisplayError> {
        log::info!("Entering wait_until_idle");
        let mut elapsed_time = 0;
        while self.busy.is_low().unwrap_or(true) {
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

        self.rst.set_high().unwrap();
        delay.delay_ms(RESET_DELAY_MS.into());

        self.rst.set_low().unwrap();
        delay.delay_ms(RESET_DELAY_MS.into());

        self.rst.set_high().unwrap();
        delay.delay_ms(RESET_DELAY_MS.into());

        log::info!("Device reset complete, waiting for idle");
        self.wait_until_idle(delay);
    }
}
