//! Text rendering utilities for SSD1680 display
//!
//! This module provides high-level text rendering functions that handle
//! multiline text automatically, positioning, and styling.

use embedded_graphics::mono_font::{iso_8859_15::FONT_5X8, MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::{prelude::*, text::Text};
use crate::ssd1680::graphics::{Display, DisplayRotation};

/// Text alignment options
#[derive(Clone, Copy, Debug, Default)]
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
pub struct TextRenderer;

impl TextRenderer {
    /// Write text of any length with automatic multiline handling
    ///
    /// This function automatically wraps text to fit within the display width,
    /// handles multiple lines, and applies the specified alignment.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `text` - The text to render (can contain newlines)
    /// * `x` - Starting X coordinate
    /// * `y` - Starting Y coordinate
    /// * `max_width` - Maximum width for text wrapping (0 = use display width)
    /// * `config` - Text rendering configuration
    ///
    /// # Returns
    ///
    /// Returns the total height of the rendered text in pixels
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ssd1680::text::{TextRenderer, TextConfig};
    /// use embedded_graphics::pixelcolor::BinaryColor;
    ///
    /// let config = TextConfig::new(&FONT_5X8)
    ///     .color(BinaryColor::On)
    ///     .alignment(TextAlignment::Center);
    ///
    /// let height = TextRenderer::write_text(
    ///     &mut display,
    ///     "Hello\nWorld!",
    ///     10,
    ///     20,
    ///     0, // Auto-detect width
    ///     config,
    /// )?;
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
        
        // Determine the maximum width for wrapping
        let display_width = display.size().width;
        let wrap_width = if max_width == 0 {
            display_width
        } else {
            max_width.min(display_width)
        };

        // Split text into lines (handle both explicit \n and automatic wrapping)
        let lines = Self::wrap_text(text, config.font, wrap_width);

        // Calculate total height
        let font_height = config.font.character_size.height;
        let line_height = font_height * config.line_spacing;
        let total_height = lines.len() as u32 * line_height;

        // Render each line
        for (i, line) in lines.iter().enumerate() {
            let line_y = y + (i as u32 * line_height);
            
            // Calculate X position based on alignment
            let line_x = match config.alignment {
                TextAlignment::Left => x,
                TextAlignment::Center => {
                    let text_width = Self::measure_text_width(line, config.font);
                    x + (wrap_width.saturating_sub(text_width)) / 2
                }
                TextAlignment::Right => {
                    let text_width = Self::measure_text_width(line, config.font);
                    x + wrap_width.saturating_sub(text_width)
                }
            };

            // Draw the line
            Text::new(line, Point::new(line_x as i32, line_y as i32), text_style)
                .draw(display)?;
        }

        Ok(total_height)
    }

    /// Write a single line of text (simplified version for single-line text)
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `text` - The text to render (single line, no newlines)
    /// * `x` - X coordinate
    /// * `y` - Y coordinate
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
        
        // Calculate X position based on alignment if needed
        let final_x = match config.alignment {
            TextAlignment::Left => x,
            TextAlignment::Center | TextAlignment::Right => {
                // For single line, we need the display width to calculate alignment
                let text_width = Self::measure_text_width(text, config.font);
                let display_width = display.size().width;
                match config.alignment {
                    TextAlignment::Center => x + display_width.saturating_sub(text_width) / 2,
                    TextAlignment::Right => x + display_width.saturating_sub(text_width),
                    _ => x,
                }
            }
        };

        Text::new(text, Point::new(final_x as i32, y as i32), text_style)
            .draw(display)?;

        Ok(())
    }

    /// Clear an area of the display and write text in it
    ///
    /// This function clears a rectangular area and then writes text within it.
    /// Useful for updating dynamic content like status messages.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `text` - The text to render
    /// * `x` - Top-left X coordinate of the area
    /// * `y` - Top-left Y coordinate of the area
    /// * `width` - Width of the area to clear
    /// * `height` - Height of the area to clear
    /// * `config` - Text rendering configuration
    pub fn write_text_in_area<D>(
        display: &mut D,
        text: &str,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        config: TextConfig,
    ) -> Result<(), D::Error>
    where
        D: Display + DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        // Clear the area first
        Self::clear_area(display, x, y, width, height, BinaryColor::Off)?;

        // Write the text within the area
        Self::write_text(display, text, x, y, width, config)?;

        Ok(())
    }

    /// Clear a rectangular area of the display
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `x` - Top-left X coordinate
    /// * `y` - Top-left Y coordinate
    /// * `width` - Width of the area
    /// * `height` - Height of the area
    /// * `color` - Fill color
    pub fn clear_area<D>(
        display: &mut D,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        color: BinaryColor,
    ) -> Result<(), D::Error>
    where
        D: Display + DrawTarget<Color = BinaryColor> + OriginDimensions,
    {
        use embedded_graphics::primitives::Rectangle;
        
        let rect = Rectangle::new(
            Point::new(x as i32, y as i32),
            Size::new(width, height),
        );
        
        display.fill_solid(&rect, color)
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
        text.chars()
            .map(|_c| font.character_size.width)
            .sum()
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
    fn wrap_text(text: &str, font: &MonoFont, max_width: u32) -> Vec<String> {
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
    use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
    use embedded_graphics::mock_display::MockDisplay;
    use crate::ssd1680::graphics::Display2in13;

    #[test]
    fn test_text_config_default() {
        let config = TextConfig::default();
        assert_eq!(config.font, &FONT_5X8);
        assert_eq!(config.color, BinaryColor::On);
        assert_eq!(config.alignment, TextAlignment::Left);
        assert_eq!(config.line_spacing, 1);
    }

    #[test]
    fn test_text_config_builder() {
        let config = TextConfig::new(&FONT_5X8)
            .color(BinaryColor::Off)
            .alignment(TextAlignment::Center)
            .line_spacing(2);
            
        assert_eq!(config.font, &FONT_5X8);
        assert_eq!(config.color, BinaryColor::Off);
        assert_eq!(config.alignment, TextAlignment::Center);
        assert_eq!(config.line_spacing, 2);
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
