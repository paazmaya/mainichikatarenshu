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

```sh
cargo espflash board-info

Chip type:         esp32s3 (revision v0.2)
Crystal frequency: 40 MHz
Flash size:        8MB
Features:          WiFi, BLE
```

## Partitions

Edit `partitions.csv` and check that it makes sense:

```sh
espflash partition-table partitions.csv
```

Output would be something like:

```
╭──────────┬──────┬─────────┬─────────┬────────────────────┬───────────╮
│ Name     ┆ Type ┆ SubType ┆ Offset  ┆ Size               ┆ Encrypted │
╞══════════╪══════╪═════════╪═════════╪════════════════════╪═══════════╡
│ nvs      ┆ data ┆ nvs     ┆ 0x9000  ┆ 0x10000 (64KiB)    ┆ false     │
├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ phy_init ┆ data ┆ phy     ┆ 0x19000 ┆ 0x10000 (64KiB)    ┆ false     │
├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ factory  ┆ app  ┆ factory ┆ 0x30000 ┆ 0x300000 (3072KiB) ┆ false     │
╰──────────┴──────┴─────────┴─────────┴────────────────────┴───────────╯
```

Convert the CSV file to a binary:

```sh
espflash partition-table --to-binary --output partition-table.bin partitions.csv
```


## Boot loader

https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/bootloader.html

Initial bootloader was:

```
ESP-IDF v5.1-beta1-378-gea5e0ff298-dirt 2nd stage bootloader
```

Started by installing for Windows
https://dl.espressif.com/dl/esp-idf/

Choosing the latest version (5.4. at the time, released just two days ago) and installing it in `C:\Espressif\frameworks\esp-idf-v5.4`.

```sh
idf.py create-project kukkuu
cd kukkuu
```

Then using "menuconfig" to create some defaults:

```sh
idf.py menuconfig
```

Editing `sdkconfig` to `CONFIG_IDF_TARGET="esp32s3"` and `CONFIG_ESPTOOLPY_FLASHSIZE_8MB=y`.

Then needed to clean first

```sh
idf.py fullclean
```

Finally creating the boot loader:

```sh
idf.py bootloader
```

How does the bootloader image look like?

```sh
esptool.py image_info .\build\bootloader\bootloader.bin          
```
```       
esptool.py v4.8.1
File size: 22352 (bytes)
Detected image type: ESP32-S3
Image version: 1
Entry point: 403c899c
4 segments

Segment 1: len 0x0164c load 0x3fce2810 file_offs 0x00000018 [BYTE_ACCESSIBLE,MEM_INTERNAL,DRAM]
Segment 2: len 0x00004 load 0x403c8700 file_offs 0x0000166c [MEM_INTERNAL,IRAM]
Segment 3: len 0x00d7c load 0x403c8704 file_offs 0x00001678 [MEM_INTERNAL,IRAM]
Segment 4: len 0x03320 load 0x403cb700 file_offs 0x000023fc [MEM_INTERNAL,IRAM]
Checksum: 96 (valid)
Validation Hash: 2527a34d14de8e76959bd7d8c7429d897cc37831c2c6774515a4074df043c74f (valid)
```

Flashing is easy via same tool:

```sh
idf.py bootloader-flash
```

Output, repetitive parts omitted:

```
Executing action: bootloader-flash
Serial port COM4
Detecting chip type... ESP32-S3
Bootloader binary size 0x5750 bytes. 0xa8b0 bytes (66%) free.
esptool.py v4.8.1
Chip is ESP32-S3 (QFN56) (revision v0.2)
Features: WiFi, BLE, Embedded PSRAM 8MB (AP_3v3)
Crystal is 40MHz
Changing baud rate to 460800
Configuring flash size...
Flash will be erased from 0x00000000 to 0x00005fff...
SHA digest in image updated
Compressed 22352 bytes to 14245...
Writing at 0x00000000... (100 %)
Wrote 22352 bytes (14245 compressed) at 0x00000000 in 0.6 seconds (effective 298.6 kbit/s)...
Hash of data verified.
```

For a full day I was flashing and flashing, changing configuraytion in ESP-IDF project, but once flashing with
`espflash`, the ouput told that the older bootloader was still being used.

Finally it turned out that espflash has its own bootloader provided when not using esp-idf-sys package in the project.

```
runner = "espflash flash --monitor --bootloader kukkuu/build/bootloader/bootloader.bin --partition-table partition-table.bin"
```

Now the output from `cargo +esp run --release` is

```
ESP-ROM:esp32s3-20210327
Build:Mar 27 2021
rst:0x1 (POWERON),boot:0xb (SPI_FAST_FLASH_BOOT)
SPIWP:0xee
mode:DIO, clock div:2
load:0x3fce2820,len:0x16ec
load:0x403c8700,len:0x4
load:0x403c8704,len:0xeac
load:0x403cb700,len:0x3170
entry 0x403c894c
I (31) boot: ESP-IDF v5.4-dirty 2nd stage bootloader
I (31) boot: Multicore bootloader
I (31) boot: chip revision: v0.2
I (31) boot: efuse block revision: v1.3
I (34) qio_mode: Enabling default flash chip QIO
I (38) boot.esp32s3: Boot SPI Speed : 40MHz
I (42) boot.esp32s3: SPI Mode       : QIO
I (46) boot.esp32s3: SPI Flash Size : 4MB
I (50) boot: Enabling RNG early entropy source...
I (54) boot: Partition Table:
I (57) boot: ## Label            Usage          Type ST Offset   Length
I (63) boot:  0 nvs              WiFi data        01 02 00009000 00010000
I (70) boot:  1 phy_init         RF data          01 01 00019000 00010000
I (76) boot:  2 factory          factory app      00 00 00030000 00300000
I (83) boot: End of partition table
I (86) boot: Loaded app from partition at offset 0x30000
I (91) boot: Disabling RNG early entropy source...
```  


## License

MIT
