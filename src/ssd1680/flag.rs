pub struct Flag;
impl Flag {
    pub const DATA_ENTRY_INCRY_INCRX: u8 = 0b11;
    pub const INTERNAL_TEMP_SENSOR: u8 = 0x80;
    pub const BORDER_WAVEFORM_FOLLOW_LUT: u8 = 0b0100;
    pub const BORDER_WAVEFORM_LUT1: u8 = 0b0001;
    pub const DISPLAY_MODE_1: u8 = 0xF7;
}
