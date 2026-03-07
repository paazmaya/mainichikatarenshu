//! Application-specific display functions
//!
//! This module contains display functions specific to the kata reminder application,
//! separated from the generic display utilities to improve testability and maintainability.

use crate::ssd1680::graphics::Display2in13;
use crate::ssd1680::text::{TextRenderer, TextConfig, TextAlignment};
use crate::ssd1680::display_utils::{DisplayManager, presets};
use embedded_graphics::pixelcolor::BinaryColor;
use display_interface::DisplayError;
use alloc::format;

/// Kata display manager for application-specific display operations
pub struct KataDisplayManager;

impl KataDisplayManager {
    /// Display the main kata reminder screen
    ///
    /// This function displays the main screen showing current date, time,
    /// and a kata reminder message.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `date_str` - Current date string
    /// * `time_str` - Current time string
    /// * `kata_name` - Name of the kata to practice
    /// * `wifi_status` - Optional WiFi status message
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_kata_reminder(
        display: &mut Display2in13,
        date_str: &str,
        time_str: &str,
        kata_name: &str,
        wifi_status: Option<&str>,
    ) -> Result<(), DisplayError> {
        // Clear and prepare display
        DisplayManager::clear_and_prepare(display)?;

        // Show WiFi status if provided
        if let Some(status) = wifi_status {
            DisplayManager::show_status_message(display, status, Some(presets::status()))?;
        }

        // Show date and time
        DisplayManager::show_datetime_status(display, date_str, time_str, None)?;

        // Show kata reminder with emphasis
        let kata_config = presets::large_centered();
        let kata_title = presets::title();
        
        // Add "Today's Kata:" label
        TextRenderer::write_line(
            display,
            "Today's Kata:",
            10,
            120,
            kata_title,
        )?;

        // Display the kata name centered and prominent
        TextRenderer::write_text(
            display,
            kata_name,
            0, // Center horizontally
            150, // Position below the title
            0, // Auto-detect width
            kata_config,
        )?;

        Ok(())
    }

    /// Display a completion confirmation screen
    ///
    /// This function displays a confirmation message when the user
    /// completes their kata practice.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `kata_name` - Name of the completed kata
    /// * `completion_time` - Time when completed
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_completion_screen(
        display: &mut Display2in13,
        kata_name: &str,
        completion_time: &str,
    ) -> Result<(), DisplayError> {
        DisplayManager::clear_and_prepare(display)?;

        // Show completion message
        let completion_config = presets::large_centered();
        
        TextRenderer::write_text(
            display,
            &format!("✓ Completed\n{}\n\nTime: {}", kata_name, completion_time),
            0,
            50, // Start further down to center the message
            0,
            completion_config,
        )?;

        Ok(())
    }

    /// Display a motivational message
    ///
    /// This function displays motivational training messages.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `message` - The motivational message
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_motivational_message(
        display: &mut Display2in13,
        message: &str,
    ) -> Result<(), DisplayError> {
        DisplayManager::clear_and_prepare(display)?;

        let config = presets::large_centered();
        DisplayManager::show_centered_message(display, message, Some(config))?;

        Ok(())
    }

    /// Display training statistics
    ///
    /// This function displays training statistics and progress.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `stats` - Training statistics to display
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_training_stats(
        display: &mut Display2in13,
        stats: &TrainingStats,
    ) -> Result<(), DisplayError> {
        DisplayManager::clear_and_prepare(display)?;

        // Title
        TextRenderer::write_line(
            display,
            "Training Progress",
            10,
            10,
            presets::title(),
        )?;

        // Statistics
        let stats_text = format!(
            "Total Sessions: {}\nThis Week: {}\nCurrent Streak: {} days\nBest Streak: {} days",
            stats.total_sessions,
            stats.weekly_sessions,
            stats.current_streak,
            stats.best_streak
        );

        TextRenderer::write_text(
            display,
            &stats_text,
            10,
            50,
            0,
            presets::body(),
        )?;

        Ok(())
    }

    /// Display a menu or options screen
    ///
    /// This function displays a menu with selectable options.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `title` - Menu title
    /// * `options` - List of menu options
    /// * `selected_index` - Index of the currently selected option
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_menu(
        display: &mut Display2in13,
        title: &str,
        options: &[&str],
        selected_index: usize,
    ) -> Result<(), DisplayError> {
        DisplayManager::clear_and_prepare(display)?;

        // Title
        TextRenderer::write_line(display, title, 10, 10, presets::title())?;

        // Menu options
        let mut y_position = 50;
        for (i, option) in options.iter().enumerate() {
            let config = if i == selected_index {
                // Highlight selected option
                TextConfig::new(presets::body().font)
                    .color(BinaryColor::On)
                    .alignment(TextAlignment::Left)
            } else {
                // Dim unselected options
                TextConfig::new(presets::body().font)
                    .color(BinaryColor::Off) // White (dimmed on this display)
                    .alignment(TextAlignment::Left)
            };

            let prefix = if i == selected_index { "> " } else { "  " };
            let menu_text = format!("{}{}", prefix, option);

            TextRenderer::write_line(display, &menu_text, 10, y_position, config)?;
            y_position += 20; // Spacing between options
        }

        Ok(())
    }

    /// Display an error or informational message
    ///
    /// This function displays error messages or important information.
    ///
    /// # Arguments
    ///
    /// * `display` - The display buffer to draw on
    /// * `title` - Message title (e.g., "Error", "Info")
    /// * `message` - The message content
    /// * `is_error` - Whether this is an error message (affects styling)
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn show_message(
        display: &mut Display2in13,
        title: &str,
        message: &str,
        is_error: bool,
    ) -> Result<(), DisplayError> {
        DisplayManager::clear_and_prepare(display)?;

        // Title with appropriate styling
        let title_config = if is_error {
            TextConfig::new(presets::title().font)
                .color(BinaryColor::On) // Black for error
                .alignment(TextAlignment::Center)
        } else {
            presets::title()
        };

        TextRenderer::write_line(display, title, 0, 30, title_config)?;

        // Message content
        let message_config = presets::centered();
        TextRenderer::write_text(display, message, 0, 80, 0, message_config)?;

        Ok(())
    }
}

/// Training statistics structure
#[derive(Debug, Clone, Default)]
pub struct TrainingStats {
    /// Total number of training sessions
    pub total_sessions: u32,
    /// Sessions completed this week
    pub weekly_sessions: u32,
    /// Current consecutive days streak
    pub current_streak: u32,
    /// Best streak achieved
    pub best_streak: u32,
}

impl TrainingStats {
    /// Create a new training statistics structure
    pub fn new(
        total_sessions: u32,
        weekly_sessions: u32,
        current_streak: u32,
        best_streak: u32,
    ) -> Self {
        Self {
            total_sessions,
            weekly_sessions,
            current_streak,
            best_streak,
        }
    }

    /// Update statistics after completing a session
    pub fn session_completed(&mut self) {
        self.total_sessions += 1;
        self.weekly_sessions += 1;
        self.current_streak += 1;
        
        if self.current_streak > self.best_streak {
            self.best_streak = self.current_streak;
        }
    }

    /// Reset weekly statistics (call on Sunday night or Monday morning)
    pub fn reset_weekly(&mut self) {
        self.weekly_sessions = 0;
    }

    /// Break streak (call when a day is missed)
    pub fn break_streak(&mut self) {
        self.current_streak = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;

    #[test]
    fn test_training_stats_creation() {
        let stats = TrainingStats::new(10, 3, 5, 7);
        assert_eq!(stats.total_sessions, 10);
        assert_eq!(stats.weekly_sessions, 3);
        assert_eq!(stats.current_streak, 5);
        assert_eq!(stats.best_streak, 7);
    }

    #[test]
    fn test_training_stats_session_completed() {
        let mut stats = TrainingStats::new(10, 3, 5, 7);
        stats.session_completed();
        
        assert_eq!(stats.total_sessions, 11);
        assert_eq!(stats.weekly_sessions, 4);
        assert_eq!(stats.current_streak, 6);
        assert_eq!(stats.best_streak, 7); // Unchanged
    }

    #[test]
    fn test_training_stats_new_best_streak() {
        let mut stats = TrainingStats::new(10, 3, 5, 7);
        stats.session_completed();
        stats.session_completed();
        stats.session_completed();
        
        assert_eq!(stats.best_streak, 8); // Updated to new best
    }

    #[test]
    fn test_training_stats_reset_weekly() {
        let mut stats = TrainingStats::new(10, 3, 5, 7);
        stats.reset_weekly();
        
        assert_eq!(stats.weekly_sessions, 0);
        assert_eq!(stats.total_sessions, 10); // Unchanged
        assert_eq!(stats.current_streak, 5); // Unchanged
    }

    #[test]
    fn test_training_stats_break_streak() {
        let mut stats = TrainingStats::new(10, 3, 5, 7);
        stats.break_streak();
        
        assert_eq!(stats.current_streak, 0);
        assert_eq!(stats.best_streak, 7); // Unchanged
    }
}
