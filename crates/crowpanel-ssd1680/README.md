# crowpanel-ssd1680

`no_std` driver for the SSD1680 e-paper controller on the
[CrowPanel ESP32 2.9" E-paper HMI Display](https://www.elecrow.com/crowpanel-esp32-2-9-e-paper-hmi-display-with-128-296-resolution-black-white-color-driven-by-spi-interface.html)
(128×296 pixels, black and white, SPI).

It uses the `embedded-hal` 1.0 traits, does not allocate and provides an
`embedded-graphics` frame buffer.

## Usage

```rust,ignore
use crowpanel_ssd1680::{Display, Display2in9, Ssd1680};
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

let mut panel = Ssd1680::new(spi, busy, dc, rst, delay)?;

let mut frame = Display2in9::new();
frame.clear(BinaryColor::Off)?; // white
// draw with embedded-graphics, BinaryColor::On is black

panel.display_frame(frame.buffer())?; // blocks for a few seconds
panel.sleep()?;                       // next display_frame wakes the panel
```

## API

| Item                             | Purpose                                                        |
| -------------------------------- | -------------------------------------------------------------- |
| `Ssd1680::new`                   | Creates the driver and initialises the controller              |
| `Ssd1680::init`                  | Hardware/software reset and panel initialisation code          |
| `Ssd1680::display_frame`         | Initialises, writes a frame, refreshes and waits for BUSY      |
| `Ssd1680::sleep`                 | Enters deep sleep mode 1; the panel keeps its image            |
| `Display2in9`                    | Frame buffer implementing `DrawTarget<Color = BinaryColor>`    |
| `Display`, `DisplayRotation`     | Buffer access and drawing rotation                             |
| `Cmd`, `Flag`                    | SSD1680 command codes and the parameter values the driver uses |
| `Pins`                           | GPIO numbers of the CrowPanel board                            |
| `WIDTH`, `HEIGHT`, `FRAME_BYTES` | Panel geometry                                                 |

## Hardware notes

Based on `info/SSD1680_datasheet.pdf` (section 9.1 and the command table) and the
CrowPanel schematic in this repository:

- Each frame follows the datasheet flow: hardware reset, software reset (`0x12`),
  initialisation code (`0x01`, `0x11`, `0x44`, `0x45`, `0x3C`, `0x18`, `0x4E`, `0x4F`),
  write RAM (`0x24`), then refresh (`0x22` + `0x20`) and wait for BUSY.
- The refresh uses Display Update Control 2 = `0xF4`. It loads the temperature and the
  waveform LUT from OTP, so the driver never writes a LUT. The commonly quoted `0xC7`
  does not refresh this panel correctly.
- After the refresh the second RAM bank (`0x26`) is filled with white.
- Deep sleep can only be left with a hardware reset, which is why `display_frame`
  always re-initialises.
- BUSY is polled every 10 ms for up to 30 s. A timeout returns
  `DisplayError::BusWriteError`.
- E-paper pins: CS GPIO45, DC GPIO46, RST GPIO47, BUSY GPIO48, SCK GPIO12, MOSI GPIO11.
  Panel power is switched by GPIO7 (`LCD_3.3_CTL`).

## Testing

Unit tests live inline in each module. The driver tests record the exact SPI and pin
traffic of `new`, `display_frame` and `sleep`, so any change to the bytes sent to the
panel shows up as a test failure.

```sh
cargo +stable test -p crowpanel-ssd1680 --target <host-triple>
```

The application in the repository root covers integration testing.
