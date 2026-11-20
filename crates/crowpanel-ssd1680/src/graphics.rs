//! Frame buffer that implements the `embedded-graphics` [`DrawTarget`].

use crate::color::Color;
use crate::{FRAME_BYTES, HEIGHT, WIDTH};
use display_interface::DisplayError;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

/// Rotation of the drawing coordinates relative to the panel RAM
#[derive(Clone, Copy, Default)]
pub enum DisplayRotation {
    /// No rotation
    #[default]
    Rotate0,
    /// Rotate by 90 degrees clockwise
    Rotate90,
    /// Rotate by 180 degrees clockwise
    Rotate180,
    /// Rotate 270 degrees clockwise
    Rotate270,
}

/// Necessary traits for all displays to implement for drawing
///
/// Adds support for:
/// - Drawing (With the help of DrawTarget/Embedded Graphics)
/// - Rotations
/// - Clearing
pub trait Display: DrawTarget {
    /// Clears the buffer of the display with the chosen background color
    fn clear_buffer(&mut self, background_color: Color) {
        let fill_color = if self.is_inverted() {
            background_color.inverse().byte_value()
        } else {
            background_color.byte_value()
        };

        for elem in self.buffer_mut().iter_mut() {
            *elem = fill_color
        }
    }

    /// Returns the buffer
    fn buffer(&self) -> &[u8];

    /// Returns a mutable buffer
    fn buffer_mut(&mut self) -> &mut [u8];

    /// Sets the rotation of the display
    fn set_rotation(&mut self, rotation: DisplayRotation);

    /// Get the current rotation of the display
    fn rotation(&self) -> DisplayRotation;

    /// If the color for this display is inverted
    fn is_inverted(&self) -> bool;

    /// Sets a single pixel in the buffer, ignoring pixels outside the display
    fn draw_helper(
        &mut self,
        width: u32,
        height: u32,
        pixel: Pixel<BinaryColor>,
    ) -> Result<(), Self::Error> {
        let rotation = self.rotation();
        let is_inverted = self.is_inverted();
        let buffer = self.buffer_mut();

        let Pixel(point, color) = pixel;
        if outside_display(point, width, height, rotation) {
            return Ok(());
        }

        // Give us index inside the buffer and the bit-position in that u8 which needs to be changed
        let (index, bit) = find_position(point.x as u32, point.y as u32, width, height, rotation);
        let index = index as usize;

        // "Draw" the Pixel on that bit
        match color {
            // Black
            BinaryColor::On => {
                if is_inverted {
                    buffer[index] |= bit;
                } else {
                    buffer[index] &= !bit;
                }
            }
            // White
            BinaryColor::Off => {
                if is_inverted {
                    buffer[index] &= !bit;
                } else {
                    buffer[index] |= bit;
                }
            }
        }
        Ok(())
    }
}

/// Frame buffer for the 2.9" 128×296 panel
///
/// `BinaryColor::On` draws black and `BinaryColor::Off` draws white.
pub struct Display2in9 {
    buffer: [u8; FRAME_BYTES],
    rotation: DisplayRotation,
    is_inverted: bool,
}

impl Default for Display2in9 {
    fn default() -> Self {
        Self::new()
    }
}

impl Display2in9 {
    /// Creates a white frame buffer
    pub fn new() -> Self {
        Display2in9 {
            buffer: [Color::White.byte_value(); FRAME_BYTES],
            rotation: DisplayRotation::default(),
            is_inverted: false,
        }
    }
}

impl DrawTarget for Display2in9 {
    type Error = DisplayError;
    type Color = BinaryColor;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for p in pixels.into_iter() {
            self.draw_helper(WIDTH.into(), HEIGHT.into(), p)?;
        }
        Ok(())
    }
}

impl OriginDimensions for Display2in9 {
    fn size(&self) -> Size {
        //if display is rotated 90 deg or 270 then swap height and width
        match self.rotation() {
            DisplayRotation::Rotate0 | DisplayRotation::Rotate180 => {
                Size::new(WIDTH.into(), HEIGHT.into())
            }
            DisplayRotation::Rotate90 | DisplayRotation::Rotate270 => {
                Size::new(HEIGHT.into(), WIDTH.into())
            }
        }
    }
}

impl Display for Display2in9 {
    fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    fn set_rotation(&mut self, rotation: DisplayRotation) {
        self.rotation = rotation;
    }

    fn rotation(&self) -> DisplayRotation {
        self.rotation
    }

    fn is_inverted(&self) -> bool {
        self.is_inverted
    }
}

// Checks if a pos is outside the defined display
fn outside_display(p: Point, width: u32, height: u32, rotation: DisplayRotation) -> bool {
    if p.x < 0 || p.y < 0 {
        return true;
    }
    let (x, y) = (p.x as u32, p.y as u32);
    match rotation {
        DisplayRotation::Rotate0 | DisplayRotation::Rotate180 => {
            if x >= width || y >= height {
                return true;
            }
        }
        DisplayRotation::Rotate90 | DisplayRotation::Rotate270 => {
            if y >= width || x >= height {
                return true;
            }
        }
    }
    false
}

fn find_rotation(x: u32, y: u32, width: u32, height: u32, rotation: DisplayRotation) -> (u32, u32) {
    match rotation {
        DisplayRotation::Rotate0 => (x, y),
        DisplayRotation::Rotate90 => (width - 1 - y, x),
        DisplayRotation::Rotate180 => (width - 1 - x, height - 1 - y),
        DisplayRotation::Rotate270 => (y, height - 1 - x),
    }
}

#[rustfmt::skip]
//returns index position in the u8-slice and the bit-position inside that u8
fn find_position(x: u32, y: u32, width: u32, height: u32, rotation: DisplayRotation) -> (u32, u8) {
    let (nx, ny) = find_rotation(x, y, width, height, rotation);
    (
        nx / 8 + width.div_ceil(8) * ny,
        0x80 >> (nx % 8),
    )
}

#[cfg(test)]
mod tests {
    use super::{find_position, outside_display, Display, Display2in9, DisplayRotation};
    use crate::color::Color;
    use embedded_graphics::{
        pixelcolor::BinaryColor, prelude::*, primitives::Line, primitives::PrimitiveStyle,
    };

    #[test]
    fn buffer_clear() {
        let mut display = Display2in9::new();

        for &byte in display.buffer.iter() {
            assert_eq!(byte, Color::White.byte_value());
        }

        display.clear_buffer(Color::Black);

        for &byte in display.buffer.iter() {
            assert_eq!(byte, Color::Black.byte_value());
        }
    }

    #[test]
    fn rotation_overflow() {
        use crate::{HEIGHT, WIDTH};
        let width = WIDTH as u32;
        let height = HEIGHT as u32;
        test_rotation_overflow(width, height, DisplayRotation::Rotate0);
        test_rotation_overflow(width, height, DisplayRotation::Rotate90);
        test_rotation_overflow(width, height, DisplayRotation::Rotate180);
        test_rotation_overflow(width, height, DisplayRotation::Rotate270);
    }

    fn test_rotation_overflow(width: u32, height: u32, rotation2: DisplayRotation) {
        let max_value = width / 8 * height;
        for x in 0..(width + height) {
            //limit x because it runs too long
            for y in 0..(u32::MAX) {
                if outside_display(Point::new(x as i32, y as i32), width, height, rotation2) {
                    break;
                } else {
                    let (idx, _) = find_position(x, y, width, height, rotation2);
                    assert!(idx < max_value);
                }
            }
        }
    }

    #[test]
    fn graphics_rotation_0() {
        let mut display = Display2in9::new();

        let _ = Line::new(Point::new(0, 0), Point::new(7, 0))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut display);

        let buffer = display.buffer();

        assert_eq!(buffer[0], Color::Black.byte_value());

        for &byte in buffer.iter().skip(1) {
            assert_eq!(byte, Color::White.byte_value());
        }
    }
}
