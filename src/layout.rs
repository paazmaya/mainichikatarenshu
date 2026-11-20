//! Generic drawing helpers and text presets shared by all screens.
//!
//! Every function only draws into a frame buffer; sending the buffer to the
//! panel is the caller's responsibility.

use crate::text::{TextAlignment, TextConfig, TextRenderer};
use crowpanel_ssd1680::{Display2in9, DisplayError};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{DrawTarget, OriginDimensions};

/// Horizontal padding between text and the display edge, in pixels.
pub const MARGIN: u32 = 10;

/// Clear the whole buffer to white, the panel's background colour.
pub fn clear(display: &mut Display2in9) -> Result<(), DisplayError> {
    display.clear(BinaryColor::Off)
}

/// Draw a message centred horizontally and vertically.
///
/// Long lines are wrapped to the display width.
pub fn draw_centered_message(display: &mut Display2in9, message: &str) -> Result<(), DisplayError> {
    let config = presets::centered();
    let display_size = display.size();
    let lines = TextRenderer::wrap_text(message, config.font, display_size.width).len() as u32;
    let line_height = config.font.character_size.height * config.line_spacing;
    let top = display_size.height.saturating_sub(lines * line_height) / 2;
    TextRenderer::write_text(display, message, 0, top + config.font.baseline, 0, config)?;
    Ok(())
}

/// Draw labelled date and time values at the top of the display.
///
/// The time block is omitted when `time` is empty.
pub fn draw_datetime(
    display: &mut Display2in9,
    date: &str,
    time: &str,
) -> Result<(), DisplayError> {
    TextRenderer::write_line(display, "Current Date:", MARGIN, 10, presets::body())?;
    TextRenderer::write_line(display, date, MARGIN, 30, presets::title())?;
    if !time.is_empty() {
        TextRenderer::write_line(display, "Current Time:", MARGIN, 60, presets::body())?;
        TextRenderer::write_line(display, time, MARGIN, 80, presets::title())?;
    }
    Ok(())
}

/// Predefined text configurations for common use cases.
pub mod presets {
    use super::*;
    use embedded_graphics::mono_font::iso_8859_15::{FONT_10X20, FONT_5X8};

    /// Large, left-aligned text for titles and values.
    pub fn title() -> TextConfig {
        TextConfig::new(&FONT_10X20)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Left)
    }

    /// Small, left-aligned text for labels and body copy.
    pub fn body() -> TextConfig {
        TextConfig::new(&FONT_5X8)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Left)
    }

    /// Small, right-aligned text for status values.
    pub fn status() -> TextConfig {
        TextConfig::new(&FONT_5X8)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Right)
    }

    /// Small, centred text for notices.
    pub fn centered() -> TextConfig {
        TextConfig::new(&FONT_5X8)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Center)
    }

    /// Large, centred text for the featured kata name.
    pub fn large_centered() -> TextConfig {
        TextConfig::new(&FONT_10X20)
            .color(BinaryColor::On)
            .alignment(TextAlignment::Center)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use crowpanel_ssd1680::{Display, WIDTH};

    fn render(draw: impl FnOnce(&mut Display2in9) -> Result<(), DisplayError>) -> Vec<u8> {
        let mut display = Display2in9::new();
        draw(&mut display).unwrap();
        display.buffer().to_vec()
    }

    #[test]
    fn centered_message_draws_within_display() {
        let buffer = render(|d| draw_centered_message(d, "Setup required"));
        let changed_columns: Vec<_> = buffer
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte != 0xff).then_some(index % (WIDTH as usize / 8)))
            .collect();
        assert!(!changed_columns.is_empty());
        assert!(changed_columns
            .iter()
            .all(|column| (3..=12).contains(column)));
    }

    #[test]
    fn centered_message_is_vertically_centered() {
        let buffer = render(|d| draw_centered_message(d, "Setup required"));
        let row_bytes = WIDTH as usize / 8;
        let rows: Vec<_> = buffer
            .chunks(row_bytes)
            .enumerate()
            .filter(|(_, row)| row.iter().any(|byte| *byte != 0xff))
            .map(|(row, _)| row)
            .collect();
        let middle = (rows[0] + rows[rows.len() - 1]) / 2;
        assert!(
            (140..=156).contains(&middle),
            "text centred at row {middle}"
        );
    }

    #[test]
    fn datetime_draws_only_provided_parts() {
        let blank = render(clear);
        let date_only = render(|d| draw_datetime(d, "2026-11-20", ""));
        let with_time = render(|d| draw_datetime(d, "2026-11-20", "06:00"));

        assert_ne!(date_only, blank);
        assert_ne!(with_time, date_only);
    }
}
