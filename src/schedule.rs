//! Helsinki-local daily schedule, independent of the device clock.

use chrono::{DateTime, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Europe::Helsinki;

/// Whether training is currently available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// Between 07:00 and 23:00 Helsinki local time.
    Active,
    /// Before 07:00 or from 23:00 Helsinki local time.
    Inactive,
}

/// Current phase and the next transition, represented as UTC.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Schedule {
    pub phase: Phase,
    pub local_date: NaiveDate,
    pub next_transition: DateTime<Utc>,
}

/// The clock is outside the supported date range, or a transition is ambiguous.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduleError;

/// Compute the schedule using a trusted UTC timestamp.
///
/// The caller must not pass an uninitialized or otherwise untrusted clock.
pub fn at(now: DateTime<Utc>) -> Result<Schedule, ScheduleError> {
    let local = now.with_timezone(&Helsinki);
    let date = local.date_naive();
    let hour = local.hour();
    let (phase, transition_date, transition_hour) = match hour {
        0..=6 => (Phase::Inactive, date, 7),
        7..=22 => (Phase::Active, date, 23),
        _ => (Phase::Inactive, date.succ_opt().ok_or(ScheduleError)?, 7),
    };
    let time = transition_date
        .and_hms_opt(transition_hour, 0, 0)
        .ok_or(ScheduleError)?;
    let transition = Helsinki
        .from_local_datetime(&time)
        .single()
        .ok_or(ScheduleError)?
        .with_timezone(&Utc);
    Ok(Schedule {
        phase,
        local_date: date,
        next_transition: transition,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn transitions_use_local_time_in_winter_and_summer() {
        let before = at(utc(2026, 1, 5, 4, 59)).unwrap();
        assert_eq!(before.phase, Phase::Inactive);
        assert_eq!(before.next_transition, utc(2026, 1, 5, 5, 0));
        let active = at(utc(2026, 7, 5, 4, 0)).unwrap();
        assert_eq!(active.phase, Phase::Active);
        assert_eq!(active.next_transition, utc(2026, 7, 5, 20, 0));
        let night = at(utc(2026, 7, 5, 20, 0)).unwrap();
        assert_eq!(night.phase, Phase::Inactive);
        assert_eq!(night.next_transition, utc(2026, 7, 6, 4, 0));
    }

    #[test]
    fn night_sleep_accounts_for_dst_transitions() {
        let spring = utc(2026, 3, 28, 21, 0);
        let autumn = utc(2026, 10, 24, 20, 0);
        assert_eq!(
            at(spring).unwrap().next_transition - spring,
            Duration::hours(7)
        );
        assert_eq!(
            at(autumn).unwrap().next_transition - autumn,
            Duration::hours(9)
        );
    }
}
