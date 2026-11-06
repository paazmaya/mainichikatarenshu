//! Daily training state machine. Persistence is the caller's responsibility.

use chrono::{DateTime, NaiveDate, Utc};
use heapless::{String, Vec};
use serde::{Deserialize, Serialize};

use crate::schedule::{self, Phase};

/// Maximum UTF-8 byte length of a kata name.
pub const MAX_KATA_BYTES: usize = 64;
/// Maximum number of completed days awaiting synchronization.
pub const MAX_PENDING_RECORDS: usize = 32;

/// A day's selected kata and its optional confirmation time.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Record {
    pub date: NaiveDate,
    pub kata: String<MAX_KATA_BYTES>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

/// A persistable snapshot of the training state.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TrainingState {
    active: Option<Record>,
    last_started: Option<NaiveDate>,
    last_observed: Option<DateTime<Utc>>,
    pending: Vec<Record, MAX_PENDING_RECORDS>,
}

/// An event requiring the caller to update its display or persist state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Change {
    None,
    Started,
    Finished,
    FinishedAndStarted,
    Confirmed,
}

/// The request could not be applied; the state remains unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidSchedule,
    ClockWentBackwards,
    MissingKata,
    InvalidKata,
    QueueFull,
    NoActiveTraining,
    OutOfOrderAcknowledgement,
}

impl TrainingState {
    pub fn active(&self) -> Option<&Record> {
        self.active.as_ref()
    }

    pub fn pending(&self) -> &[Record] {
        &self.pending
    }

    /// Advance the schedule. Supply a randomly selected kata if a new day starts.
    ///
    /// Persist the resulting snapshot atomically before acting on the returned
    /// change. A missed day with no selection is not invented retroactively.
    pub fn advance(&mut self, now: DateTime<Utc>, kata: Option<&str>) -> Result<Change, Error> {
        let mut next = self.clone();
        let change = next.advance_inner(now, kata)?;
        *self = next;
        Ok(change)
    }

    fn advance_inner(&mut self, now: DateTime<Utc>, kata: Option<&str>) -> Result<Change, Error> {
        self.check_time(now)?;
        let schedule = schedule::at(now).map_err(|_| Error::InvalidSchedule)?;
        let finish = self.active.as_ref().is_some_and(|active| {
            active.date < schedule.local_date
                || (active.date == schedule.local_date && schedule.phase == Phase::Inactive)
        });
        let start = schedule.phase == Phase::Active
            && self.last_started != Some(schedule.local_date)
            && self
                .active
                .as_ref()
                .is_none_or(|active| active.date != schedule.local_date);

        let new_record = if start {
            let name = kata.ok_or(Error::MissingKata)?;
            if name.trim().is_empty() {
                return Err(Error::InvalidKata);
            }
            let name = String::try_from(name).map_err(|_| Error::InvalidKata)?;
            Some(Record {
                date: schedule.local_date,
                kata: name,
                confirmed_at: None,
            })
        } else {
            None
        };

        if finish {
            let record = self.active.take().ok_or(Error::NoActiveTraining)?;
            self.pending.push(record).map_err(|_| Error::QueueFull)?;
        }
        if let Some(record) = new_record {
            self.last_started = Some(record.date);
            self.active = Some(record);
        }
        self.last_observed = Some(now);
        Ok(match (finish, start) {
            (false, false) => Change::None,
            (false, true) => Change::Started,
            (true, false) => Change::Finished,
            (true, true) => Change::FinishedAndStarted,
        })
    }

    /// Confirm the selected kata once during its active local day.
    pub fn confirm(&mut self, now: DateTime<Utc>) -> Result<Change, Error> {
        self.check_time(now)?;
        let schedule = schedule::at(now).map_err(|_| Error::InvalidSchedule)?;
        let active = self.active.as_mut().ok_or(Error::NoActiveTraining)?;
        if schedule.phase != Phase::Active || active.date != schedule.local_date {
            return Err(Error::NoActiveTraining);
        }
        let change = if active.confirmed_at.is_none() {
            active.confirmed_at = Some(now);
            Change::Confirmed
        } else {
            Change::None
        };
        self.last_observed = Some(now);
        Ok(change)
    }

    /// Remove the oldest record only after the remote service has acknowledged it.
    pub fn acknowledge(&mut self, date: NaiveDate) -> Result<Record, Error> {
        if self
            .pending
            .first()
            .is_none_or(|record| record.date != date)
        {
            return Err(Error::OutOfOrderAcknowledgement);
        }
        Ok(self.pending.remove(0))
    }

    fn check_time(&self, now: DateTime<Utc>) -> Result<(), Error> {
        if self.last_observed.is_some_and(|previous| now < previous) {
            Err(Error::ClockWentBackwards)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn utc(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, day, hour, 0, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn daily_selection_confirmation_and_retry_are_idempotent() {
        let mut state = TrainingState::default();
        let start = utc(5, 5);
        assert_eq!(state.advance(start, Some("Kata A")), Ok(Change::Started));
        assert_eq!(state.advance(start, Some("Kata B")), Ok(Change::None));
        let confirmed = start + Duration::hours(3);
        assert_eq!(state.confirm(confirmed), Ok(Change::Confirmed));
        assert_eq!(
            state.confirm(confirmed + Duration::hours(1)),
            Ok(Change::None)
        );
        assert_eq!(state.advance(utc(5, 21), None), Ok(Change::Finished));
        assert_eq!(state.advance(utc(5, 21), None), Ok(Change::None));
        assert_eq!(state.pending().len(), 1);
        assert_eq!(state.pending()[0].kata, "Kata A");
        assert_eq!(state.pending()[0].confirmed_at, Some(confirmed));
        assert_eq!(
            state.acknowledge(state.pending()[0].date.succ_opt().unwrap()),
            Err(Error::OutOfOrderAcknowledgement)
        );
        assert_eq!(state.pending().len(), 1);
        let date = state.pending()[0].date;
        state.acknowledge(date).unwrap();
        assert!(state.pending().is_empty());
    }

    #[test]
    fn missed_shutdown_finalizes_before_next_start() {
        let mut state = TrainingState::default();
        state.advance(utc(5, 5), Some("Kata A")).unwrap();
        assert_eq!(
            state.advance(utc(6, 5), Some("Kata B")),
            Ok(Change::FinishedAndStarted)
        );
        assert_eq!(state.pending()[0].confirmed_at, None);
        assert_eq!(state.active().unwrap().kata, "Kata B");
    }

    #[test]
    fn invalid_input_and_backward_clock_leave_state_unchanged() {
        let mut state = TrainingState::default();
        let snapshot = state.clone();
        assert_eq!(state.advance(utc(5, 5), None), Err(Error::MissingKata));
        assert_eq!(state.advance(utc(5, 5), Some(" ")), Err(Error::InvalidKata));
        assert_eq!(state, snapshot);
        state.advance(utc(5, 5), Some("Kata A")).unwrap();
        let snapshot = state.clone();
        assert_eq!(state.confirm(utc(5, 4)), Err(Error::ClockWentBackwards));
        assert_eq!(state, snapshot);
    }

    #[test]
    fn full_queue_does_not_discard_the_active_record() {
        let mut state = TrainingState::default();
        state.advance(utc(5, 5), Some("Kata A")).unwrap();
        let record = state.active().unwrap().clone();
        for _ in 0..MAX_PENDING_RECORDS {
            state.pending.push(record.clone()).unwrap();
        }
        let snapshot = state.clone();
        assert_eq!(state.advance(utc(5, 21), None), Err(Error::QueueFull));
        assert_eq!(state, snapshot);
    }
}
