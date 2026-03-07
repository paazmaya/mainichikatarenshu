//! Display utilities and helper functions
//!
//! This module provides reusable utilities for common display operations,
//! reducing code duplication and improving maintainability.

use embedded_graphics::pixelcolor::BinaryColor;
use crate::ssd1680::{WIDTH, HEIGHT};
use crate::ssd1680::graphics::{Display, Display2in13, DisplayRotation};
use crate::ssd1680::text::{TextRenderer, TextConfig, TextAlignment};
use display_interface::DisplayError;
use embedded_graphics::prelude::{Primitive, DrawTarget};
use embedded_graphics::Drawable;

/// Display manager that provides high-level display operations
pub struct DisplayManager;

impl DisplayManager {
    /// Create a new display buffer with standard initialization
    ///
    /// This function creates a new display buffer and applies common
    /// initialization settings used throughout the application.
    ///
    /// # Returns
    ///
    /// A new display buffer ready for drawing
    pub fn create_display() -> Display2in13 {
        let mut display = Display2in13::new();
        
        // Set default rotation to match physical display orientation
        display.set_rotation(DisplayRotation::Rotate270);
        
        display
    }

    /// Clear the display and prepare it for new content
    ///
    /// This function clears the display buffer to white (the default
    /// background color for this display) and returns the cleared buffer.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to clear
    ///
    /// # Returns
    ///
    /// The cleared display buffer (for chaining)
    pub fn clear_and_prepare(display: &mut Display2in13) -> Result<(), DisplayError> {
        // Clear to white (Off = white for this display)
        display.clear(BinaryColor::Off)?;
        Ok(())
    }

    /// Display a status message with automatic positioning
    ///
    /// This function displays a status message in the top-right corner
    /// of the display, which is commonly used for WiFi status, error messages,
    /// or other status indicators.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `message` - The status message to display
    /// * `config` - Optional text configuration (uses default if None)
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_status_message(
        display: &mut Display2in13,
        message: &str,
        config: Option<TextConfig>,
    ) -> Result<(), DisplayError> {
        let text_config = config.unwrap_or_else(|| {
            TextConfig::default()
                .alignment(TextAlignment::Right)
        });

        // Position in top-right corner with some padding
        let x = WIDTH - 10; // Right padding
        let y = 10; // Top padding

        TextRenderer::write_line(display, message, x as u32, y as u32, text_config)
    }

    /// Display a title and content with automatic layout
    ///
    /// This function displays a title at the top of the display and
    /// content below it, with proper spacing and alignment.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `title` - The title to display
    /// * `content` - The content text (can be multiline)
    /// * `config` - Optional text configuration
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_title_and_content(
        display: &mut Display2in13,
        title: &str,
        content: &str,
        config: Option<TextConfig>,
    ) -> Result<(), DisplayError> {
        let base_config = config.unwrap_or_default();
        
        // Draw title at the top
        let title_config = TextConfig::new(base_config.font)
            .color(base_config.color)
            .alignment(TextAlignment::Left);
            
        TextRenderer::write_line(display, title, 10, 10, title_config)?;

        // Draw content below the title with spacing
        let content_config = TextConfig::new(base_config.font)
            .color(base_config.color)
            .alignment(base_config.alignment)
            .line_spacing(base_config.line_spacing);

        let title_height = base_config.font.character_size.height;
        let content_y = 10 + title_height + 5; // 5 pixels spacing

        TextRenderer::write_text(display, content, 10, content_y, 0, content_config)?;

        Ok(())
    }

    /// Display a centered message (useful for notifications)
    ///
    /// This function displays a message centered both horizontally
    /// and vertically on the display.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `message` - The message to display (can be multiline)
    /// * `config` - Optional text configuration
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_centered_message(
        display: &mut Display2in13,
        message: &str,
        config: Option<TextConfig>,
    ) -> Result<(), DisplayError> {
        let text_config = config.unwrap_or_else(|| {
            TextConfig::default()
                .alignment(TextAlignment::Center)
        });

        // Calculate approximate center position
        let display_width = WIDTH;
        let display_height = HEIGHT;
        
        // Estimate text height for centering
        let lines = message.lines().count();
        let font_height = text_config.font.character_size.height;
        let line_height = font_height * text_config.line_spacing;
        let total_text_height = lines as u32 * line_height;
        
        let center_x = display_width as u32 / 2;
        let center_y = (display_height.saturating_sub(total_text_height as u16)) as u32 / 2;

        TextRenderer::write_text(display, message, center_x, center_y, 0, text_config)?;

        Ok(())
    }

    /// Display date and time information
    ///
    /// This function displays date/time information in a standard format,
    /// commonly used for the main display screen.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `date_str` - Date string (e.g., "2025-10-14")
    /// * `time_str` - Time string (e.g., "18:00")
    /// * `status_msg` - Optional status message (e.g., WiFi status)
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_datetime_status(
        display: &mut Display2in13,
        date_str: &str,
        time_str: &str,
        status_msg: Option<&str>,
    ) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::iso_8859_15::{FONT_5X8, FONT_10X20};

        // Draw date in large font
        let date_config = TextConfig::new(&FONT_10X20)
            .color(BinaryColor::On);
            
        TextRenderer::write_line(display, "Current Date:", 10, 10, 
            TextConfig::new(&FONT_5X8).color(BinaryColor::On))?;
            
        TextRenderer::write_line(display, date_str, 10, 30, date_config)?;

        // Draw time in medium font if provided
        if !time_str.is_empty() {
            let time_config = TextConfig::new(&FONT_10X20)
                .color(BinaryColor::On);
                
            TextRenderer::write_line(display, "Current Time:", 10, 60,
                TextConfig::new(&FONT_5X8).color(BinaryColor::On))?;
                
            TextRenderer::write_line(display, time_str, 10, 80, time_config)?;
        }

        // Draw status message if provided
        if let Some(status) = status_msg {
            Self::show_status_message(display, status, None)?;
        }

        Ok(())
    }

    /// Create a test pattern display
    ///
    /// This function creates a simple test pattern to verify display
    /// functionality. Useful for debugging and hardware testing.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn create_test_pattern(display: &mut Display2in13) -> Result<(), DisplayError> {
        use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
        use embedded_graphics::primitives::{Line, PrimitiveStyle};

        // Clear display first
        Self::clear_and_prepare(display)?;

        // Draw border
        let border_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
        let border = Line::new(
            embedded_graphics::prelude::Point::new(5, 5),
            embedded_graphics::prelude::Point::new(WIDTH as i32 - 5, 5),
        )
        .into_styled(border_style);
        border.draw(display)?;

        // Draw test text
        let test_config = TextConfig::new(&FONT_5X8)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Center);

        TextRenderer::write_text(
            display,
            "TEST PATTERN\nDisplay Driver\nWorking Correctly",
            0,
            (HEIGHT / 2 - 20).into(),
            0,
            test_config,
        )?;

        Ok(())
    }
}

/// Predefined text configurations for common use cases
pub mod presets {
    use super::*;
    use embedded_graphics::mono_font::iso_8859_15::{FONT_5X8, FONT_10X20};

    /// Get a text configuration for titles (large font)
    pub fn title() -> TextConfig {
        TextConfig::new(&FONT_10X20)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Left)
    }

    /// Get a text configuration for body text (small font)
    pub fn body() -> TextConfig {
        TextConfig::new(&FONT_5X8)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Left)
            .line_spacing(1)
    }

    /// Get a text configuration for status messages (small, right-aligned)
    pub fn status() -> TextConfig {
        TextConfig::new(&FONT_5X8)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Right)
    }

    /// Get a text configuration for centered text
    pub fn centered() -> TextConfig {
        TextConfig::new(&FONT_5X8)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Center)
    }

    /// Get a text configuration for large centered text
    pub fn large_centered() -> TextConfig {
        TextConfig::new(&FONT_10X20)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Center)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;

    #[test]
    fn test_display_manager_create_display() {
        let display = DisplayManager::create_display();
        // Test that display is created successfully
        // (actual testing would require checking the display buffer)
    }

    #[test]
    fn test_presets() {
        let title_config = presets::title();
        let body_config = presets::body();
        let status_config = presets::status();
        let centered_config = presets::centered();
        let large_centered_config = presets::large_centered();

        // Verify configurations have expected properties
        assert_eq!(title_config.alignment, TextAlignment::Left);
        assert_eq!(status_config.alignment, TextAlignment::Right);
        assert_eq!(centered_config.alignment, TextAlignment::Center);
        assert_eq!(large_centered_config.alignment, TextAlignment::Center);
    }
}
