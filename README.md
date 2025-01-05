# mainichikatarenshu

> 毎日型練習して！

> Every day a martial arts form training!

E-ink display showing a kata for each day, along with current electricity price, using ESP-32 and written in Rust language.

Each day at 07:00 EET the e-ink screen will wake up and update the kata name from a random list.

The display will shutdown at 23:00 EET, which will record the kata either incomplete if the button has not been pressed before that.

The knowledge of the kata being confirmed or not, will be send to Google Drive, to store the information in a spreadsheet. The columns date, kata name, confirmed (boolean), time of confirmation.

```sh
cargo install espup # Get ESP tooling handler
espup install # Install ESP tools
```

## Parts

[CrowPanel ESP32 2.9" E-paper HMI Display with 128*296 Resolution, Black/White Color Driven By SPI Interface](https://www.elecrow.com/crowpanel-esp32-2-9-e-paper-hmi-display-with-128-296-resolution-black-white-color-driven-by-spi-interface.html)

This has internal ESP32-S3 chip, packaged with the E-paper display.

The "info" folder contains datasheets and further details.

### Features

*    2.9-inch E-Paper display, 128*296 resolution, black and white, using SPI interface communication;
*    ESP32-S3 as the main chip, frequency up to 240MHz;
*    Support full viewing angle, clearly visible from any angle;
*    High contrast and high reflectivity， can provide better visual performance;
*    Pure reflection mode, no backlight required, completely relying on light reflection to display content, and the displayed content will not be lost even if the power is off;
*    Hard-coated anti-glare display surface, it can keep the content clearly visible even under direct sunlight;
*    Support Arduino IDE, ESP IDF, and MicroPython development environment to get a smooth development experience；
*    Ultra-low power consumption and partial refresh function, significantly saving energy;
*    Rich buttons and interfaces (such as GPIO interface, UART interface, home button, etc.) for easy development and operation.

2.9-inch Display Port |	Pin Number
--------------------- | -------------
MENU Button           |		IO2
Rotary Switch         |		Down(IO4); Up(IO6); CONF(IO5)
EXIT Button           |		IO1
GPIO                  |		IO3; IO9; IO15; IO17; IO19; IO21; IO8; IO14; IO16; IO18; IO20; IO38
SD Card Slot(SPI)     |		MOSI(IO40); MISO(IO13); CLK(IO39); CS(IO10)


## License

MIT
