pub struct Cmd;
impl Cmd {
    // Init
    pub const SW_RESET: u8 = 0x12;
    pub const DRIVER_CONTROL: u8 = 0x01;
    pub const DATA_ENTRY_MODE: u8 = 0x11;
    pub const TEMP_CONTROL: u8 = 0x18;
    pub const BORDER_WAVEFORM_CONTROL: u8 = 0x3C;
    pub const DISPLAY_UPDATE_CONTROL: u8 = 0x21;
    pub const SET_RAMXPOS: u8 = 0x44;
    pub const SET_RAMYPOS: u8 = 0x45;
    pub const DEEP_SLEEP_MODE: u8 = 0x10;

    // Update
    pub const SET_RAMX_COUNTER: u8 = 0x4E;
    pub const SET_RAMY_COUNTER: u8 = 0x4F;
    pub const WRITE_BW_DATA: u8 = 0x24;
    pub const WRITE_CLEAR_DATA: u8 = 0x26;
    pub const UPDATE_DISPLAY_CTRL2: u8 = 0x22;
    pub const MASTER_ACTIVATE: u8 = 0x20;
}

/*
Arduino example code had these:
0x12 - Software Reset
0x01 - Driver Output Control
0x11 - Data Entry Mode
0x18 - Temperature Sensor Control
0x3C - Border Waveform Control
0x21 - Display Update Control
0x22 - Display Update Sequence Control
0x20 - Master Activation
0x44 - Set RAM X Address Start/End
0x45 - Set RAM Y Address Start/End
0x4E - Set RAM X Address Counter
0x4F - Set RAM Y Address Counter
0x24 - Write RAM
0x26 - Write RAM for Red
*/
