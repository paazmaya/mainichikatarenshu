pub struct Flag;
impl Flag {
    pub const DATA_ENTRY_INCRY_INCRX: u8 = 0b11;
    pub const INTERNAL_TEMP_SENSOR: u8 = 0x80;
    pub const BORDER_WAVEFORM_FOLLOW_LUT: u8 = 0b0100;
    pub const BORDER_WAVEFORM_LUT1: u8 = 0b0001;
    pub const DISPLAY_MODE_1: u8 = 0xF7;
    pub const DISPLAY_UPDATE_FULL: u8 = 0xF4;
    pub const DISPLAY_UPDATE_FAST: u8 = 0xB1;
    pub const DISPLAY_UPDATE_PARTIAL_1: u8 = 0x91;
    pub const DISPLAY_UPDATE_PARTIAL_2: u8 = 0xC7;
}

/*
Arduino example code had these:
0x01 - Border Waveform Control (Follow LUT, LUT1)
0x03 - Data Entry Mode (Increment Y, Increment X)
0x80 - Internal Temperature Sensor
0x91 - Display Update Sequence Control (Partial Update)
0xB1 - Display Update Sequence Control (Fast Update)
0xC7 - Display Update Sequence Control (Partial Update)
0xF4 - Display Update Sequence Control (Full Update)
*/
