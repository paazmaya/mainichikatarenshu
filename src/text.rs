//! Text rendering utilities for SSD1680 display
//!
//! This module provides high-level text rendering functions that handle
//! multiline text automatically, positioning, and styling.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crowpanel_ssd1680::Display;
use embedded_graphics::mono_font::{iso_8859_15::FONT_5X8, MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::{prelude::*, text::Text};

/// Text alignment options
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TextAlignment {
    /// Align text to the left edge
    #[default]
    Left,
    /// Center text horizontally
    Center,
    /// Align text to the right edge
    Right,
}

/// Text rendering configuration
#[derive(Clone, Debug)]
pub struct TextConfig {
    /// Font to use for rendering
    pub font: &'static MonoFont<'static>,
    /// Text color
    pub color: BinaryColor,
    /// Horizontal alignment
    pub alignment: TextAlignment,
    /// Line spacing in pixels (multiplier of font height)
    pub line_spacing: u32,
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            font: &FONT_5X8,
            color: BinaryColor::On,
            alignment: TextAlignment::Left,
            line_spacing: 1,
        }
    }
}

impl TextConfig {
    /// Create a new text configuration with the specified font
    pub fn new(font: &'static MonoFont<'static>) -> Self {
        Self {
            font,
            ..Default::default()
        }
    }

    /// Set the text color
    pub fn color(mut self, color: BinaryColor) -> Self {
        self.color = color;
        self
    }

    /// Set the text alignment
    pub fn alignment(mut self, alignment: TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set the line spacing (multiplier of font height)
    pub fn line_spacing(mut self, line_spacing: u32) -> Self {
        self.line_spacing = line_spacing;
        self
    }
}

/// Text renderer that provides high-level text rendering functions
///
/// Text is laid out inside a horizontal box that starts at `x` and extends to
/// the right edge of the display (or `max_width` pixels, if smaller). The
/// alignment positions each line within that box, so aligned text never
/// starts past the display edge. The `y` coordinate is the baseline of the
/// first line.
pub struct TextRenderer;

impl TextRenderer {
    /// Write text with explicit newlines and automatic word wrapping.
    ///
    /// Lines are wrapped to the box width and aligned within it.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `text` - The text to render (can contain newlines)
    /// * `x` - Left edge of the text box
    /// * `y` - Baseline of the first line
    /// * `max_width` - Maximum box width (0 = up to the display edge)
    /// * `config` - Text rendering configuration
    ///
    /// # Returns
    ///
    /// The total height of the rendered text in pixels
    ///
    /// # Examples
    ///
    /// ```rust
    /// use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
    /// use mainichikatarenshu::{Display2in9, TextAlignment, TextConfig, TextRenderer};
    ///
    /// let mut display = Display2in9::new();
    /// let config = TextConfig::new(&FONT_5X8).alignment(TextAlignment::Center);
    /// let height = TextRenderer::write_text(&mut display, "Hello\nWorld!", 10, 20, 0, config)?;
    /// assert_eq!(height, 2 * FONT_5X8.character_size.height);
    /// # Ok::<(), crowpanel_ssd1680::DisplayError>(())
    /// ```
    pub fn write_text<D>(
        display: &mut D,
        text: &str,
        x: u32,
        y: u32,
        max_width: u32,
        config: TextConfig,
    ) -> Result<u32, D::Error>
    where
        D: Display + DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        let text_style = MonoTextStyle::new(config.font, config.color);
        let box_width = Self::box_width(display.size().width, x, max_width);
        let lines = Self::wrap_text(text, config.font, box_width);
        let line_height = config.font.character_size.height * config.line_spacing;

        for (i, line) in lines.iter().enumerate() {
            let line_y = y + (i as u32 * line_height);
            let line_x = Self::aligned_x(
                config.alignment,
                x,
                box_width,
                Self::measure_text_width(line, config.font),
            );
            Text::new(line, Point::new(line_x as i32, line_y as i32), text_style).draw(display)?;
        }

        Ok(lines.len() as u32 * line_height)
    }

    /// Write a single line of text without wrapping.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `text` - The text to render (single line, no newlines)
    /// * `x` - Left edge of the text box, which extends to the display edge
    /// * `y` - Baseline of the line
    /// * `config` - Text rendering configuration
    pub fn write_line<D>(
        display: &mut D,
        text: &str,
        x: u32,
        y: u32,
        config: TextConfig,
    ) -> Result<(), D::Error>
    where
        D: Display + DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        let text_style = MonoTextStyle::new(config.font, config.color);
        let line_x = Self::aligned_x(
            config.alignment,
            x,
            Self::box_width(display.size().width, x, 0),
            Self::measure_text_width(text, config.font),
        );
        Text::new(text, Point::new(line_x as i32, y as i32), text_style).draw(display)?;
        Ok(())
    }

    /// Width of the text box starting at `x`, limited by the display edge.
    fn box_width(display_width: u32, x: u32, max_width: u32) -> u32 {
        let available = display_width.saturating_sub(x);
        if max_width == 0 {
            available
        } else {
            max_width.min(available)
        }
    }

    /// X coordinate of a line of `text_width` pixels aligned in the box `[x, x + box_width)`.
    ///
    /// Text wider than the box starts at `x`.
    pub fn aligned_x(alignment: TextAlignment, x: u32, box_width: u32, text_width: u32) -> u32 {
        let free = box_width.saturating_sub(text_width);
        match alignment {
            TextAlignment::Left => x,
            TextAlignment::Center => x + free / 2,
            TextAlignment::Right => x + free,
        }
    }

    /// Measure the width of text in pixels
    ///
    /// # Arguments
    ///
    /// * `text` - The text to measure
    /// * `font` - The font to use for measurement
    ///
    /// # Returns
    ///
    /// Width of the text in pixels
    pub fn measure_text_width(text: &str, font: &MonoFont) -> u32 {
        text.chars().map(|_c| font.character_size.width).sum()
    }

    /// Wrap text to fit within a specified width
    ///
    /// This function handles both explicit newlines and automatic word wrapping.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to wrap
    /// * `font` - The font to use for width calculation
    /// * `max_width` - Maximum width per line
    ///
    /// # Returns
    ///
    /// Vector of wrapped lines
    pub fn wrap_text(text: &str, font: &MonoFont, max_width: u32) -> Vec<String> {
        let mut lines = Vec::new();

        // First split by explicit newlines
        for paragraph in text.split('\n') {
            if paragraph.is_empty() {
                lines.push(String::new());
                continue;
            }

            // Then wrap each paragraph to fit the width
            let mut current_line = String::new();
            let mut current_width = 0u32;

            for word in paragraph.split_whitespace() {
                let word_width = Self::measure_text_width(word, font);
                let space_width = font.character_size.width;

                // Check if we need to start a new line
                if current_width == 0 {
                    current_line.push_str(word);
                    current_width = word_width;
                } else if current_width + space_width + word_width <= max_width {
                    current_line.push(' ');
                    current_line.push_str(word);
                    current_width += space_width + word_width;
                } else {
                    // Save current line and start a new one
                    if !current_line.is_empty() {
                        lines.push(current_line);
                    }
                    current_line = word.to_string();
                    current_width = word_width;
                }
            }

            // Add the last line of the paragraph
            if !current_line.is_empty() {
                lines.push(current_line);
            }
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crowpanel_ssd1680::{Display2in9, WIDTH};
    use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;

    /// Byte columns (8 px each) that contain any black pixel.
    fn inked_byte_columns(display: &Display2in9) -> Vec<usize> {
        let columns = WIDTH as usize / 8;
        let mut inked: Vec<usize> = display
            .buffer()
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte != 0xff)
            .map(|(index, _)| index % columns)
            .collect();
        inked.sort_unstable();
        inked.dedup();
        inked
    }

    #[test]
    fn aligned_x_positions_within_box() {
        assert_eq!(
            TextRenderer::aligned_x(TextAlignment::Left, 10, 100, 40),
            10
        );
        assert_eq!(
            TextRenderer::aligned_x(TextAlignment::Center, 10, 100, 40),
            40
        );
        assert_eq!(
            TextRenderer::aligned_x(TextAlignment::Right, 10, 100, 40),
            70
        );
        assert_eq!(
            TextRenderer::aligned_x(TextAlignment::Right, 10, 30, 40),
            10
        );
    }

    #[test]
    fn right_aligned_line_ends_at_display_edge() {
        let mut display = Display2in9::new();
        display.clear(BinaryColor::Off).unwrap();
        let config = TextConfig::default().alignment(TextAlignment::Right);
        TextRenderer::write_line(&mut display, "Status", 10, 20, config).unwrap();
        let inked = inked_byte_columns(&display);
        assert_eq!(inked.last(), Some(&(WIDTH as usize / 8 - 1)));
        assert!(inked.first().is_some_and(|column| *column >= 10));
    }

    #[test]
    fn centered_text_respects_box_offset() {
        let mut display = Display2in9::new();
        display.clear(BinaryColor::Off).unwrap();
        let config = TextConfig::default().alignment(TextAlignment::Center);
        // A 40 px wide box at x = 64 must keep the text in the right half.
        TextRenderer::write_text(&mut display, "abc", 64, 20, 40, config).unwrap();
        let inked = inked_byte_columns(&display);
        assert!(inked.iter().all(|column| (8..13).contains(column)));
    }

    #[test]
    fn test_measure_text_width() {
        let width = TextRenderer::measure_text_width("Hello", &FONT_5X8);
        assert_eq!(width, 5 * FONT_5X8.character_size.width);
    }

    #[test]
    fn test_wrap_text() {
        let lines = TextRenderer::wrap_text("Hello world test", &FONT_5X8, 100);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Hello world test");

        let lines = TextRenderer::wrap_text("Hello world test", &FONT_5X8, 50);
        assert!(lines.len() > 1);
    }

    #[test]
    fn test_wrap_text_with_newlines() {
        let lines = TextRenderer::wrap_text("Hello\nWorld", &FONT_5X8, 100);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "Hello");
        assert_eq!(lines[1], "World");
    }
}
