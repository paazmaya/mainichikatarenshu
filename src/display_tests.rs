//! Test utilities and examples for the refactored display system
//!
//! This module contains test examples and utilities that demonstrate
//! the improved testability of the refactored code.

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;
    use embedded_graphics::pixelcolor::BinaryColor;
    
    // Import the modules we want to test
    use crate::ssd1680::text::{TextRenderer, TextConfig, TextAlignment};
    use crate::ssd1680::display_utils::{DisplayManager, presets};
    use crate::kata_display::{KataDisplayManager, TrainingStats};

    /// Test the universal text writing function
    #[test]
    fn test_universal_text_writing() {
        let mut display = MockDisplay::new();
        
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

    /// Test multiline text handling
    #[test]
    fn test_multiline_text_handling() {
        let mut display = MockDisplay::new();
        
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

    /// Test text alignment options
    #[test]
    fn test_text_alignment() {
        let mut display = MockDisplay::new();
        
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

    /// Test display manager utilities
    #[test]
    fn test_display_manager_utilities() {
        let mut display = MockDisplay::new();
        
        // Test status message display
        let result = DisplayManager::show_status_message(
            &mut display,
            "WiFi Connected",
            Some(presets::status()),
        );
        assert!(result.is_ok(), "Status message display should succeed");
        
        // Test centered message
        let result = DisplayManager::show_centered_message(
            &mut display,
            "Centered Text",
            Some(presets::centered()),
        );
        assert!(result.is_ok(), "Centered message display should succeed");
        
        // Test title and content
        let result = DisplayManager::show_title_and_content(
            &mut display,
            "Title",
            "Content line 1\nContent line 2",
            Some(presets::body()),
        );
        assert!(result.is_ok(), "Title and content display should succeed");
    }

    /// Test kata display manager functions
    #[test]
    fn test_kata_display_manager() {
        let mut display = MockDisplay::new();
        
        // Test kata reminder display
        let result = KataDisplayManager::show_kata_reminder(
            &mut display,
            "2025-10-14",
            "18:00",
            "Heian Shodan",
            Some("WiFi Connected"),
        );
        assert!(result.is_ok(), "Kata reminder display should succeed");
        
        // Test completion screen
        let result = KataDisplayManager::show_completion_screen(
            &mut display,
            "Heian Shodan",
            "18:05",
        );
        assert!(result.is_ok(), "Completion screen display should succeed");
        
        // Test motivational message
        let result = KataDisplayManager::show_motivational_message(
            &mut display,
            "Keep up the great work!",
        );
        assert!(result.is_ok(), "Motivational message display should succeed");
        
        // Test training statistics
        let stats = TrainingStats::new(10, 3, 5, 7);
        let result = KataDisplayManager::show_training_stats(&mut display, &stats);
        assert!(result.is_ok(), "Training statistics display should succeed");
        
        // Test menu display
        let options = vec!["Option 1", "Option 2", "Option 3"];
        let result = KataDisplayManager::show_menu(&mut display, "Menu", &options, 1);
        assert!(result.is_ok(), "Menu display should succeed");
        
        // Test message display
        let result = KataDisplayManager::show_message(
            &mut display,
            "Info",
            "This is an informational message",
            false,
        );
        assert!(result.is_ok(), "Info message display should succeed");
        
        let result = KataDisplayManager::show_message(
            &mut display,
            "Error",
            "This is an error message",
            true,
        );
        assert!(result.is_ok(), "Error message display should succeed");
    }

    /// Test training statistics functionality
    #[test]
    fn test_training_statistics_functionality() {
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

    /// Test text configuration presets
    #[test]
    fn test_text_configuration_presets() {
        // Test all presets
        let title_config = presets::title();
        let body_config = presets::body();
        let status_config = presets::status();
        let centered_config = presets::centered();
        let large_centered_config = presets::large_centered();
        
        // Verify configurations have expected properties
        assert_eq!(title_config.alignment, TextAlignment::Left);
        assert_eq!(body_config.alignment, TextAlignment::Left);
        assert_eq!(status_config.alignment, TextAlignment::Right);
        assert_eq!(centered_config.alignment, TextAlignment::Center);
        assert_eq!(large_centered_config.alignment, TextAlignment::Center);
        
        // Verify colors are set correctly
        assert_eq!(title_config.color, BinaryColor::On);
        assert_eq!(body_config.color, BinaryColor::On);
        assert_eq!(status_config.color, BinaryColor::On);
        assert_eq!(centered_config.color, BinaryColor::On);
        assert_eq!(large_centered_config.color, BinaryColor::On);
    }

    /// Test text wrapping functionality
    #[test]
    fn test_text_wrapping() {
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

    /// Test text measurement
    #[test]
    fn test_text_measurement() {
        use embedded_graphics::mono_font::iso_8859_15::FONT_5X8;
        
        let text = "Hello";
        let width = TextRenderer::measure_text_width(text, &FONT_5X8);
        let expected_width = text.len() as u32 * FONT_5X8.character_size.width;
        
        assert_eq!(width, expected_width, "Text width measurement should be accurate");
        
        // Test empty string
        let empty_width = TextRenderer::measure_text_width("", &FONT_5X8);
        assert_eq!(empty_width, 0, "Empty string should have zero width");
    }
}

/// Integration test example showing how to test the complete display pipeline
#[cfg(test)]
mod integration_tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;
    
    /// Test the complete kata reminder display pipeline
    #[test]
    fn test_complete_kata_reminder_pipeline() {
        let mut display = MockDisplay::new();
        
        // Simulate the complete pipeline used in main()
        let date = "2025-10-14";
        let time = "18:00";
        let kata = "Heian Shodan";
        let wifi_status = "WiFi Connected";
        
        // Create and clear display
        let mut display = DisplayManager::create_display();
        DisplayManager::clear_and_prepare(&mut display).unwrap();
        
        // Show WiFi status
        DisplayManager::show_status_message(&mut display, wifi_status, Some(presets::status())).unwrap();
        
        // Show kata reminder
        KataDisplayManager::show_kata_reminder(
            &mut display,
            date,
            time,
            kata,
            Some(wifi_status),
        ).unwrap();
        
        // At this point, the display buffer would contain the complete kata reminder
        // In a real test, you could verify specific pixels or buffer content
    }
}
