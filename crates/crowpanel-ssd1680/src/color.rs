//! Black and white colors of the panel.

/// Black/White colors
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    /// Black color
    Black,
    /// White color
    White,
}

impl Color {
    /// Value of a single pixel bit in display RAM.
    pub fn bit_value(self) -> u8 {
        match self {
            Color::White => 1,
            Color::Black => 0,
        }
    }

    /// Value of a byte that sets eight pixels to this color.
    pub fn byte_value(self) -> u8 {
        match self {
            Color::White => 0xFF,
            Color::Black => 0x00,
        }
    }

    /// Returns the opposite color.
    pub fn inverse(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

impl From<u8> for Color {
    /// Converts a pixel bit value. Panics for anything other than 0 or 1.
    fn from(value: u8) -> Self {
        match value {
            0 => Color::Black,
            1 => Color::White,
            e => panic!("Color only parses 0 and 1 (Black and White) and not `{e}`"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_u8() {
        assert_eq!(Color::Black, Color::from(0u8));
        assert_eq!(Color::White, Color::from(1u8));
    }

    #[test]
    fn from_u8_panics_for_other_values() {
        for val in 2..=u8::MAX {
            let result = std::panic::catch_unwind(|| Color::from(val));
            assert!(result.is_err());
        }
    }

    #[test]
    fn bit_value_round_trips() {
        for color in [Color::Black, Color::White] {
            assert_eq!(Color::from(color.bit_value()), color);
        }
    }

    #[test]
    fn inverse_swaps_colors() {
        assert_eq!(Color::Black.inverse(), Color::White);
        assert_eq!(Color::White.inverse(), Color::Black);
    }
}
