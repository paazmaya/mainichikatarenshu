//! Application screens and the logic that chooses which one to show.
//!
//! [`select_screen`] is a pure decision function so it can be tested on the
//! host; [`draw_screen`] renders the chosen screen into a frame buffer.

use crate::layout::{self, presets, MARGIN};
use crate::text::TextRenderer;
use crate::training::{Error as TrainingError, Record};
use alloc::format;
use alloc::string::String;
use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Europe::Helsinki;
use crowpanel_ssd1680::{Display2in9, DisplayError};
use embedded_graphics::prelude::OriginDimensions;

/// The content the display should show.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Screen<'a> {
    /// The clock has not been set yet.
    SetTime,
    /// The training workflow is paused by an error.
    TrainingError(TrainingError),
    /// A kata is selected for the current day.
    Reminder {
        /// Time of the redraw.
        now: DateTime<Utc>,
        /// The active day's record.
        record: &'a Record,
        /// Formatted electricity price, if known.
        price: Option<&'a str>,
    },
    /// Outside active hours.
    Rest,
}

/// Choose the screen for the current application state.
///
/// `now` is `None` while the clock is not trusted.
pub fn select_screen<'a>(
    now: Option<DateTime<Utc>>,
    training_error: Option<TrainingError>,
    active: Option<&'a Record>,
    price: Option<&'a str>,
) -> Screen<'a> {
    match (now, training_error, active) {
        (None, _, _) => Screen::SetTime,
        (Some(_), Some(error), _) => Screen::TrainingError(error),
        (Some(now), None, Some(record)) => Screen::Reminder { now, record, price },
        (Some(_), None, None) => Screen::Rest,
    }
}

/// Short, user-facing description of a training error.
pub fn training_error_message(error: TrainingError) -> &'static str {
    match error {
        TrainingError::QueueFull => "Queue full: LIST / ACK",
        TrainingError::ClockWentBackwards => "Clock moved backwards",
        _ => "Training error: see serial",
    }
}

/// Helsinki-local `YYYY-MM-DD` date and `HH:MM` time strings for `now`.
pub fn helsinki_date_and_time(now: DateTime<Utc>) -> (String, String) {
    let local = now.with_timezone(&Helsinki);
    (
        format!("{}", local.date_naive()),
        format!("{:02}:{:02}", local.hour(), local.minute()),
    )
}

/// Clear the buffer and draw `screen`.
pub fn draw_screen(display: &mut Display2in9, screen: &Screen<'_>) -> Result<(), DisplayError> {
    layout::clear(display)?;
    match screen {
        Screen::SetTime => layout::draw_centered_message(display, "Set time via serial"),
        Screen::TrainingError(error) => {
            layout::draw_centered_message(display, training_error_message(*error))
        }
        Screen::Reminder { now, record, price } => {
            let (date, time) = helsinki_date_and_time(*now);
            let status = if record.confirmed_at.is_some() {
                "Completed"
            } else {
                "Press confirm"
            };
            draw_kata_reminder(display, &date, &time, &record.kata, status, *price)
        }
        Screen::Rest => layout::draw_centered_message(display, "Rest until 07:00"),
    }
}

/// Draw the daily kata reminder onto a cleared buffer.
///
/// Shows the date and time at the top, the kata name in the middle, the
/// confirmation status at the bottom left and the optional electricity price
/// at the bottom right.
pub fn draw_kata_reminder(
    display: &mut Display2in9,
    date: &str,
    time: &str,
    kata_name: &str,
    status: &str,
    electricity_price: Option<&str>,
) -> Result<(), DisplayError> {
    layout::draw_datetime(display, date, time)?;
    TextRenderer::write_line(display, "Today's Kata:", MARGIN, 120, presets::title())?;
    TextRenderer::write_text(display, kata_name, 0, 150, 0, presets::large_centered())?;
    TextRenderer::write_line(display, status, MARGIN, 278, presets::body())?;
    if let Some(price) = electricity_price {
        let width = display.size().width.saturating_sub(2 * MARGIN);
        TextRenderer::write_text(display, price, MARGIN, 290, width, presets::status())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use chrono::{NaiveDate, TimeZone};
    use crowpanel_ssd1680::Display;
    use embedded_graphics::{pixelcolor::BinaryColor, prelude::DrawTarget};

    fn render(draw: impl FnOnce(&mut Display2in9) -> Result<(), DisplayError>) -> Vec<u8> {
        let mut display = Display2in9::new();
        draw(&mut display).unwrap();
        display.buffer().to_vec()
    }

    fn record(confirmed: bool) -> Record {
        Record {
            date: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            kata: "Bassai".try_into().unwrap(),
            confirmed_at: confirmed.then(|| Utc.with_ymd_and_hms(2026, 11, 20, 8, 0, 0).unwrap()),
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 20, 4, 5, 0).unwrap()
    }

    #[test]
    fn select_screen_prioritises_clock_then_error_then_record() {
        let record = record(false);
        let error = Some(TrainingError::QueueFull);
        assert_eq!(
            select_screen(None, error, Some(&record), None),
            Screen::SetTime
        );
        assert_eq!(
            select_screen(Some(now()), error, Some(&record), None),
            Screen::TrainingError(TrainingError::QueueFull)
        );
        assert_eq!(
            select_screen(Some(now()), None, Some(&record), Some("1.0 c/kWh")),
            Screen::Reminder {
                now: now(),
                record: &record,
                price: Some("1.0 c/kWh")
            }
        );
        assert_eq!(select_screen(Some(now()), None, None, None), Screen::Rest);
    }

    #[test]
    fn training_errors_have_actionable_messages() {
        assert_eq!(
            training_error_message(TrainingError::QueueFull),
            "Queue full: LIST / ACK"
        );
        assert_eq!(
            training_error_message(TrainingError::ClockWentBackwards),
            "Clock moved backwards"
        );
        assert_eq!(
            training_error_message(TrainingError::InvalidSchedule),
            "Training error: see serial"
        );
    }

    #[test]
    fn helsinki_time_is_formatted_locally() {
        let (date, time) =
            helsinki_date_and_time(Utc.with_ymd_and_hms(2026, 11, 20, 22, 7, 0).unwrap());
        assert_eq!(date, "2026-11-21");
        assert_eq!(time, "00:07");
    }

    #[test]
    fn reminder_shows_status_and_optional_price() {
        let unconfirmed = record(false);
        let confirmed = record(true);
        let reminder = |record, price| Screen::Reminder {
            now: now(),
            record,
            price,
        };
        let plain = render(|d| draw_screen(d, &reminder(&unconfirmed, None)));
        let completed = render(|d| draw_screen(d, &reminder(&confirmed, None)));
        let priced = render(|d| draw_screen(d, &reminder(&unconfirmed, Some("12.4 c/kWh"))));

        assert_ne!(plain, render(layout::clear));
        assert_ne!(completed, plain);
        assert_ne!(priced, plain);
    }

    #[test]
    fn screens_replace_previous_content() {
        let record = record(false);
        let screens = [
            Screen::SetTime,
            Screen::TrainingError(TrainingError::ClockWentBackwards),
            Screen::Reminder {
                now: now(),
                record: &record,
                price: None,
            },
            Screen::Rest,
        ];
        for screen in &screens {
            let fresh = render(|d| draw_screen(d, screen));
            let redrawn = render(|d| {
                d.clear(BinaryColor::On)?;
                draw_screen(d, screen)
            });
            assert_eq!(redrawn, fresh);
        }
    }
}
