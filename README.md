# mainichikatarenshu

> 毎日型練習して！

> Every day a martial arts form training!

E-ink display showing a kata for each day, along with current electricity price, using ESP-32 and written in Rust language.

Each day at 07:00 EET the e-ink screen will wake up and update the kata name from a random list.

The display will shutdown at 23:00 EET, which will record the kata either incomplete if the button has not been pressed before that.

The knowledge of the kata being confirmed or not, will be send to Google Drive, to store the information in a spreadsheet. The columns date, kata name, confirmed (boolean), time of confirmation.

```sh
cargo install espup@0.11.0 # Get ESP tooling handler, version that still works in Windows: could not be opened: LoadLibraryExW failed
espup install # Install ESP tools
```

In case there was an earlier installation of esp toolchain, that folder shoulbe be removed so it is not reused.
C:\\Users\\Jukka\\.rustup\\toolchains\\esp\

https://community.chocolatey.org/packages/llvm
choco install llvm



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
│ nvs      ┆ data ┆ nvs     ┆ 0x9000  ┆ 0x8000 (32KiB)     ┆ false     │
├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ phy_init ┆ data ┆ phy     ┆ 0x11000 ┆ 0x8000 (32KiB)     ┆ false     │
├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
│ factory  ┆ app  ┆ factory ┆ 0x20000 ┆ 0x200000 (2048KiB) ┆ false     │
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

For a full day I was flashing and flashing, changing configuration in ESP-IDF project, but once flashing with
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

## Debug symbols missing in dev profile

```  
xtensa-esp32-elf-readelf -S target/xtensa-esp32s3-none-elf/debug/mainichikatarenshu
There are 4 section headers, starting at offset 0x60:

Section Headers:
  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al
  [ 0]                   NULL            00000000 000000 000000 00      0   0  0
  [ 1] .symtab           SYMTAB          00000000 000034 000010 10      2   1  4
  [ 2] .strtab           STRTAB          00000000 000044 000001 00      0   0  1
  [ 3] .shstrtab         STRTAB          00000000 000045 00001b 00      0   0  1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), p (processor specific)
``` 

At this point
idf.py erase-flash
Chip erase completed successfully in 5.0s

to see that there was all the time in use some older partition table, as now there is nothing...

Use menuconfig to set the partion table file as `CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="../partitions.csv"`. 

Ok, the bootloader and partition table flashing needs to be done via `idf.py`, and the resulted bootloader used when application is flashed via `espflash`.

Uh, there was nothing coming in the final binary, in both release and dev  profiles it was the same, no debug symbols.

Went forward by using the [esp-idf-template](https://github.com/esp-rs/esp-idf-template), but that had immediately problems linking:

```
error: linking with `ldproxy` failed: exit code: 101
  = note: [ldproxy] Running ldproxy
          thread 'main' panicked at C:\Users\Jukka\.cargo\registry\src\index.crates.io-6f17d22bba15001f\ldproxy-0.3.4\src/main.rs:44:13:
          Cannot locate argument '--ldproxy-linker <linker>'
          stack backtrace:
          note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
```

Finally this was solved by having a `build.rs` file with the contents:

```rust
fn main() {
    embuild::espidf::sysenv::output();
}
```

Now that the build is passing, checking to see what kind of symbols there are:

```
xtensa-esp32-elf-readelf -S .\target\xtensa-esp32s3-espidf\release\mainichikatarenshu
There are 37 section headers, starting at offset 0x566f34:

Section Headers:
  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al
  [ 0]                   NULL            00000000 000000 000000 00      0   0  0
  [ 1] .rtc.text         PROGBITS        600fe000 06c000 000100 00  AX  0   0  4
  [ 2] .rtc.force_fast   PROGBITS        600fe100 06c100 000020 00  WA  0   0  4
  [ 3] .rtc_noinit       PROGBITS        50000000 06c120 000000 00   W  0   0  1
  [ 4] .rtc.force_slow   PROGBITS        50000000 06c120 000000 00   W  0   0  1
  [ 5] .rtc_reserved     NOBITS          600fffe8 06cfe8 000018 00  WA  0   0  8
  [ 6] .iram0.vectors    PROGBITS        40374000 01f000 000403 00  AX  0   0  4
  [ 7] .iram0.text       PROGBITS        40374404 01f404 00dd03 00  AX  0   0  4
  [ 8] .dram0.dummy      NOBITS          3fc88000 012000 00a200 00  WA  0   0  1
  [ 9] .dram0.data       PROGBITS        3fc92200 01c200 002954 00  WA  0   0 16
  [10] .noinit           NOBITS          3fc94b54 000000 000000 00  WA  0   0  1
  [11] .dram0.bss        NOBITS          3fc94b58 01eb54 000990 00  WA  0   0  8
  [12] .flash.text       PROGBITS        42000020 02e020 03de38 00  AX  0   0  4
  [13] .flash_rodat[...] NOBITS          3c000020 001020 040000 00  WA  0   0  1
  [14] .flash.appdesc    PROGBITS        3c040020 001020 000100 00   A  0   0 16
  [15] .flash.rodata     PROGBITS        3c040120 001120 0103a8 00  WA  0   0 16
  [16] .ext_ram.dummy    NOBITS          3c000020 001020 05ffe0 00  WA  0   0  1
  [17] .iram0.text_end   NOBITS          40382107 02d107 0000f9 00  WA  0   0  1
  [18] .iram0.data       PROGBITS        40382200 06c120 000000 00   W  0   0  1
  [19] .iram0.bss        PROGBITS        40382200 06c120 000000 00   W  0   0  1
  [20] .dram0.heap_start PROGBITS        3fc954e8 06c120 000000 00   W  0   0  1
  [21] .debug_aranges    PROGBITS        00000000 06c120 007688 00      0   0  8
  [22] .debug_info       PROGBITS        00000000 0737a8 1e52b6 00      0   0  1
  [23] .debug_abbrev     PROGBITS        00000000 258a5e 02c1ef 00      0   0  1
  [24] .debug_line       PROGBITS        00000000 284c4d 11285f 00      0   0  1
  [25] .debug_frame      PROGBITS        00000000 3974ac 00cba0 00      0   0  4
  [26] .debug_str        PROGBITS        00000000 3a404c 10920e 01  MS  0   0  1
  [27] .debug_loc        PROGBITS        00000000 4ad25a 0518f6 00      0   0  1
  [28] .debug_ranges     PROGBITS        00000000 4feb50 02bc40 00      0   0  8
  [29] .debug_line_str   PROGBITS        00000000 52a790 001a9d 01  MS  0   0  1
  [30] .debug_loclists   PROGBITS        00000000 52c22d 00a6d6 00      0   0  1
  [31] .debug_rnglists   PROGBITS        00000000 536903 000240 00      0   0  1
  [32] .comment          PROGBITS        00000000 536b43 000093 01  MS  0   0  1
  [33] .xtensa.info      NOTE            00000000 536bd6 000038 00      0   0  1
  [34] .symtab           SYMTAB          00000000 536c10 013a80 10     35 1988  4
  [35] .strtab           STRTAB          00000000 54a690 01c6cc 00      0   0  1
  [36] .shstrtab         STRTAB          00000000 566d5c 0001d7 00      0   0  1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), p (processor specific)
```

```
xtensa-esp32-elf-readelf -S .\target\xtensa-esp32s3-espidf\debug\mainichikatarenshu
There are 37 section headers, starting at offset 0x10c2050:

Section Headers:
  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al
  [ 0]                   NULL            00000000 000000 000000 00      0   0  0
  [ 1] .rtc.text         PROGBITS        600fe000 120000 000100 00  AX  0   0  4
  [ 2] .rtc.force_fast   PROGBITS        600fe100 120100 000020 00  WA  0   0  4
  [ 3] .rtc_noinit       PROGBITS        50000000 120120 000000 00   W  0   0  1
  [ 4] .rtc.force_slow   PROGBITS        50000000 120120 000000 00   W  0   0  1
  [ 5] .rtc_reserved     NOBITS          600fffe8 120fe8 000018 00  WA  0   0  8
  [ 6] .iram0.vectors    PROGBITS        40374000 034000 000403 00  AX  0   0  4
  [ 7] .iram0.text       PROGBITS        40374404 034404 00f527 00  AX  0   0  4
  [ 8] .dram0.dummy      NOBITS          3fc88000 025000 00ba00 00  WA  0   0  1
  [ 9] .dram0.data       PROGBITS        3fc93a00 030a00 002b9c 00  WA  0   0 16
  [10] .noinit           NOBITS          3fc9659c 000000 000000 00  WA  0   0  1
  [11] .dram0.bss        NOBITS          3fc965a0 03359c 0009e0 00  WA  0   0  8
  [12] .flash.text       PROGBITS        42000020 044020 0dbf46 00  AX  0   0  4
  [13] .flash_rodat[...] NOBITS          3c000020 001020 0e0000 00  WA  0   0  1
  [14] .flash.appdesc    PROGBITS        3c0e0020 001020 000100 00   A  0   0 16
  [15] .flash.rodata     PROGBITS        3c0e0120 001120 023480 00  WA  0   0 16
  [16] .ext_ram.dummy    NOBITS          3c000020 001020 10ffe0 00  WA  0   0  1
  [17] .iram0.text_end   NOBITS          4038392b 04392b 0000d5 00  WA  0   0  1
  [18] .iram0.data       PROGBITS        40383a00 120120 000000 00   W  0   0  1
  [19] .iram0.bss        PROGBITS        40383a00 120120 000000 00   W  0   0  1
  [20] .dram0.heap_start PROGBITS        3fc96f80 120120 000000 00   W  0   0  1
  [21] .debug_aranges    PROGBITS        00000000 120120 031e40 00      0   0  8
  [22] .debug_info       PROGBITS        00000000 151f60 5f371d 00      0   0  1
  [23] .debug_abbrev     PROGBITS        00000000 74567d 03cc90 00      0   0  1
  [24] .debug_line       PROGBITS        00000000 78230d 22eaa0 00      0   0  1
  [25] .debug_frame      PROGBITS        00000000 9b0db0 00f3e8 00      0   0  4
  [26] .debug_str        PROGBITS        00000000 9c0198 52e7b4 01  MS  0   0  1
  [27] .debug_loc        PROGBITS        00000000 eee94c 071c80 00      0   0  1
  [28] .debug_ranges     PROGBITS        00000000 f605d0 0600d8 00      0   0  8
  [29] .debug_line_str   PROGBITS        00000000 fc06a8 001a9d 01  MS  0   0  1
  [30] .debug_loclists   PROGBITS        00000000 fc2145 00a6d6 00      0   0  1
  [31] .debug_rnglists   PROGBITS        00000000 fcc81b 000240 00      0   0  1
  [32] .comment          PROGBITS        00000000 fcca5b 000093 01  MS  0   0  1
  [33] .xtensa.info      NOTE            00000000 fccaee 000038 00      0   0  1
  [34] .symtab           SYMTAB          00000000 fccb28 032100 10     35 2614  4
  [35] .strtab           STRTAB          00000000 ffec28 0c3251 00      0   0  1
  [36] .shstrtab         STRTAB          00000000 10c1e79 0001d7 00      0   0  1
Key to Flags:
  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),
  L (link order), O (extra OS processing required), G (group), T (TLS),
  C (compressed), x (unknown), o (OS specific), E (exclude),
  D (mbind), p (processor specific)
```

Same for both.


It took some time to come back to this project as the Waveshare driver and embedded graphics seemed to be
hard to get working together.
Finally when looking at the diff 
https://github.com/caemor/epd-waveshare/compare/v0.5.0...v0.6.0
and 
https://github.com/embedded-graphics/embedded-graphics
matcing use cases, got the build to compile with
minilar text styling in a display created from `VarDisplay::new()` instead of `Display2in9::default()`.

That then revealed that even when the display has two colors, they should not be defined as `embedded_graphics::pixelcolor::BinaryColor:::Off`,
but as `epd_waveshare::color::Color::White`.

The text started to appear in the screen but they kept drawing om top of existing ones,
not clearning the screen at any point.

The working module was `epd_waveshare::epd2in9_v2`, but the internal commands might differ, which is why 
other similar board configurations were tried out...

The display is using a driver SSD1680, so perhaps should be looking at some other EPD provier than Waveshare crate.

GitHub search revealed
https://github.com/embedded-drivers/epd
and 
https://github.com/mbv/ssd1680
to be promising ones, the latter being used at https://github.com/mbv/esp32-ssd1680/blob/main/Cargo.toml

None of the drivers seemed fit, since the commands they used were not matching the ones in Arduino examples by Elecrow

Writing my own driver based on mbv/ssd1680

Replaced package `espflash v3.3.0` with `espflash v4.2.0` (executable `espflash.exe`)

After a long battle of comparing a working Arduino example and the Rust driver, finally got the screen to show something.

## License

MIT

## Dev Containers

This repository offers Dev Containers supports for:
-  [VS Code Dev Containers](https://code.visualstudio.com/docs/remote/containers#_quick-start-open-an-existing-folder-in-a-container)
-  [GitHub Codespaces](https://docs.github.com/en/codespaces/developing-in-codespaces/creating-a-codespace)
> **Note**
>
> In [order to use GitHub Codespaces](https://github.com/features/codespaces#faq)
> the project needs to be published in a GitHub repository and the user needs
> to be part of the Codespaces beta or have the project under an organization.

If using VS Code or GitHub Codespaces, you can pull the image instead of building it
from the Dockerfile by selecting the `image` property instead of `build` in
`.devcontainer/devcontainer.json`. Further customization of the Dev Container can
be achived, see [.devcontainer.json reference](https://code.visualstudio.com/docs/remote/devcontainerjson-reference).

When using Dev Containers, some tooling to facilitate building, flashing and
simulating in Wokwi is also added.

## Wokwi Simulation

### VS Code Dev Containers and GitHub Codespaces

The Dev Container includes the Wokwi Vs Code installed, hence you can simulate your built projects doing the following:
1. Press `F1`
2. Run `Wokwi: Start Simulator`

> **Note**
>
>  We assume that the project is built in `debug` mode, if you want to simulate projects in release, please update the `elf` and  `firmware` proprieties in `wokwi.toml`.

For more information and details on how to use the Wokwi extension, see [Getting Started] and [Debugging your code] Chapter of the Wokwi documentation.

[Getting Started]: https://docs.wokwi.com/vscode/getting-started
[Debugging your code]: https://docs.wokwi.com/vscode/debugging
