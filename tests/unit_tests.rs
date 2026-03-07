//! Unit tests that can run without ESP32 hardware
//!
//! These tests use mock displays and avoid hardware-specific dependencies.

use embedded_graphics::mock_display::MockDisplay;
use embedded_graphics::pixelcolor::BinaryColor;

// Import only the modules we want to test without hardware
use mainichikatarenshu::ssd1680::text::{TextRenderer, TextConfig, TextAlignment};
use mainichikatarenshu::ssd1680::graphics::Display2in13;
use mainichikatarenshu::ssd1680::color::Color;
use mainichikatarenshu::kata_display::TrainingStats;

#[test]
fn test_text_rendering_without_hardware() {
    let mut display = Display2in13::bw();
    
    // Test single line text
    let config = TextConfig::default();
    let result = TextRenderer::write_line(
        &mut display,
        "Hello World",
        10,
        20,
        config,
    );
    
    assert!(result.is_ok(), "Single line text writing should succeed");
}

#[test]
fn test_multiline_text_without_hardware() {
    let mut display = Display2in13::bw();
    
    let multiline_text = "Line 1\nLine 2\nLine 3";
    let config = TextConfig::default();
    
    let height = TextRenderer::write_text(
        &mut display,
        multiline_text,
        10,
        20,
        0, // Auto-detect width
        config,
    );
    
    assert!(height.is_ok(), "Multiline text writing should succeed");
    let text_height = height.unwrap();
    assert!(text_height > 0, "Text height should be positive");
}

#[test]
fn test_text_alignment_without_hardware() {
    let mut display = Display2in13::bw();
    
    let alignments = [
        TextAlignment::Left,
        TextAlignment::Center,
        TextAlignment::Right,
    ];
    
    for alignment in alignments {
        let config = TextConfig::default().alignment(alignment);
        let result = TextRenderer::write_line(
            &mut display,
            "Test",
            10,
            20,
            config,
        );
        
        assert!(result.is_ok(), "Text alignment {:?} should work", alignment);
    }
}

#[test]
fn test_color_conversions() {
    // Test color bit values
    assert_eq!(Color::White.get_bit_value(), 1, "White should be bit value 1");
    assert_eq!(Color::Black.get_bit_value(), 0, "Black should be bit value 0");
    
    // Test color byte values
    assert_eq!(Color::White.get_byte_value(), 0xFF, "White should be byte value 0xFF");
    assert_eq!(Color::Black.get_byte_value(), 0x00, "Black should be byte value 0x00");
    
    // Test color inversion
    assert_eq!(Color::White.inverse(), Color::Black, "White inverse should be Black");
    assert_eq!(Color::Black.inverse(), Color::White, "Black inverse should be White");
}

#[test]
fn test_training_statistics_logic() {
    let mut stats = TrainingStats::default();
    
    // Test initial state
    assert_eq!(stats.total_sessions, 0);
    assert_eq!(stats.weekly_sessions, 0);
    assert_eq!(stats.current_streak, 0);
    assert_eq!(stats.best_streak, 0);
    
    // Test session completion
    stats.session_completed();
    assert_eq!(stats.total_sessions, 1);
    assert_eq!(stats.weekly_sessions, 1);
    assert_eq!(stats.current_streak, 1);
    assert_eq!(stats.best_streak, 1);
    
    // Test multiple sessions
    stats.session_completed();
    stats.session_completed();
    assert_eq!(stats.total_sessions, 3);
    assert_eq!(stats.current_streak, 3);
    assert_eq!(stats.best_streak, 3);
    
    // Test weekly reset
    stats.reset_weekly();
    assert_eq!(stats.weekly_sessions, 0);
    assert_eq!(stats.total_sessions, 3); // Should remain unchanged
    assert_eq!(stats.current_streak, 3); // Should remain unchanged
    
    // Test streak break
    stats.break_streak();
    assert_eq!(stats.current_streak, 0);
    assert_eq!(stats.best_streak, 3); // Best streak should be preserved
}

#[test]
fn test_text_measurement_without_hardware() {
    use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
    
    let text = "Hello";
    let width = TextRenderer::measure_text_width(text, &FONT_5X8);
    let expected_width = text.len() as u32 * FONT_5X8.character_size.width;
    
    assert_eq!(width, expected_width, "Text width measurement should be accurate");
    
    // Test empty string
    let empty_width = TextRenderer::measure_text_width("", &FONT_5X8);
    assert_eq!(empty_width, 0, "Empty string should have zero width");
}

#[test]
fn test_text_wrapping_without_hardware() {
    use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
    
    let long_text = "This is a very long text that should wrap when the width is limited";
    
    // Test with sufficient width (no wrapping needed)
    let wide_lines = TextRenderer::wrap_text(long_text, &FONT_5X8, 500);
    assert_eq!(wide_lines.len(), 1, "Wide width should not require wrapping");
    
    // Test with limited width (wrapping needed)
    let narrow_lines = TextRenderer::wrap_text(long_text, &FONT_5X8, 100);
    assert!(narrow_lines.len() > 1, "Narrow width should require wrapping");
    
    // Test with explicit newlines
    let text_with_newlines = "Line 1\nLine 2\nLine 3";
    let newline_lines = TextRenderer::wrap_text(text_with_newlines, &FONT_5X8, 200);
    assert_eq!(newline_lines.len(), 3, "Explicit newlines should be preserved");
}
