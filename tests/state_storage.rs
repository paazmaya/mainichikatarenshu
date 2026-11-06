use chrono::{Duration, TimeZone, Utc};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_futures::block_on;
use embedded_storage::nor_flash::{ErrorType, NorFlash, NorFlashErrorKind, ReadNorFlash};
use mainichikatarenshu::storage::{PersistentTraining, StateStore, TransactionError};
use mainichikatarenshu::training::{Error as TrainingError, TrainingState, MAX_PENDING_RECORDS};
use std::{cell::Cell, rc::Rc};

struct Flash {
    bytes: Vec<u8>,
    fail_write: Rc<Cell<bool>>,
}

impl Flash {
    fn new() -> Self {
        Self {
            bytes: vec![0xff; 65536],
            fail_write: Rc::new(Cell::new(false)),
        }
    }
}

impl ErrorType for Flash {
    type Error = NorFlashErrorKind;
}

impl ReadNorFlash for Flash {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let start = offset as usize;
        bytes.copy_from_slice(
            self.bytes
                .get(start..start + bytes.len())
                .ok_or(NorFlashErrorKind::OutOfBounds)?,
        );
        Ok(())
    }

    fn capacity(&self) -> usize {
        self.bytes.len()
    }
}

impl NorFlash for Flash {
    const WRITE_SIZE: usize = 4;
    const ERASE_SIZE: usize = 4096;

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let start = offset as usize;
        let dest = self
            .bytes
            .get_mut(start..start + bytes.len())
            .ok_or(NorFlashErrorKind::OutOfBounds)?;
        let failing = self.fail_write.replace(false);
        let count = if failing {
            bytes.len() / 2
        } else {
            bytes.len()
        };
        for (old, new) in dest.iter_mut().zip(bytes).take(count) {
            *old &= *new;
        }
        if failing {
            return Err(NorFlashErrorKind::Other);
        }
        Ok(())
    }

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.bytes
            .get_mut(from as usize..to as usize)
            .ok_or(NorFlashErrorKind::OutOfBounds)?
            .fill(0xff);
        Ok(())
    }
}

#[test]
fn snapshot_survives_reload_and_interrupted_write() {
    let mut buffer = [0; 4096];
    let flash = Flash::new();
    let fail_write = flash.fail_write.clone();
    let mut store = StateStore::new(BlockingAsync::new(flash), 0..65536).unwrap();
    assert_eq!(block_on(store.load(&mut buffer)).unwrap(), None);
    let start = Utc.with_ymd_and_hms(2026, 1, 5, 5, 0, 0).unwrap();
    let mut state = TrainingState::default();
    state.advance(start, Some("Kata A")).unwrap();
    block_on(store.save(&mut buffer, &state)).unwrap();
    let flash = store.into_flash();
    let mut store = StateStore::new(flash, 0..65536).unwrap();
    assert_eq!(
        block_on(store.load(&mut buffer)).unwrap(),
        Some(state.clone())
    );

    state.confirm(start + Duration::hours(1)).unwrap();
    fail_write.set(true);
    assert!(block_on(store.save(&mut buffer, &state)).is_err());
    let flash = store.into_flash();
    let mut store = StateStore::new(flash, 0..65536).unwrap();
    let recovered = block_on(store.load(&mut buffer)).unwrap().unwrap();
    assert!(recovered.active().is_some());
    assert_eq!(recovered.active().unwrap().kata, "Kata A");
    assert!(recovered.active().unwrap().confirmed_at.is_none());
}

#[test]
fn maximum_queue_snapshot_fits_one_sector() {
    let mut state = TrainingState::default();
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 5, 0, 0).unwrap();
    let name = "X".repeat(64);
    for day in 0..=MAX_PENDING_RECORDS {
        let time = start + Duration::days(day as i64);
        state.advance(time, Some(&name)).unwrap();
        if day < MAX_PENDING_RECORDS {
            state.confirm(time + Duration::hours(1)).unwrap();
        }
    }
    assert_eq!(state.pending().len(), MAX_PENDING_RECORDS);
    let mut buffer = [0; 4096];
    let mut store = StateStore::new(BlockingAsync::new(Flash::new()), 0..65536).unwrap();
    block_on(store.save(&mut buffer, &state)).unwrap();
    assert_eq!(block_on(store.load(&mut buffer)).unwrap(), Some(state));
}

#[test]
fn failed_commit_does_not_publish_new_state() {
    let flash = Flash::new();
    let fail_write = flash.fail_write.clone();
    let store = StateStore::new(BlockingAsync::new(flash), 0..65536).unwrap();
    let mut buffer = [0; 4096];
    let mut app = block_on(PersistentTraining::load(store, &mut buffer)).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 1, 5, 5, 0, 0).unwrap();
    block_on(app.advance(&mut buffer, now, Some("Kata A"))).unwrap();
    let original = app.state().clone();
    fail_write.set(true);
    assert!(block_on(app.confirm(&mut buffer, now + Duration::hours(1))).is_err());
    assert_eq!(app.state(), &original);
    assert!(matches!(
        block_on(app.confirm(&mut buffer, now + Duration::hours(1))),
        Err(TransactionError::RequiresReload)
    ));
    let store = app.into_store();
    let mut app = block_on(PersistentTraining::load(store, &mut buffer)).unwrap();
    assert_eq!(app.state(), &original);
    block_on(app.confirm(&mut buffer, now + Duration::hours(1))).unwrap();
    assert!(app.state().active().unwrap().confirmed_at.is_some());
}

#[test]
fn full_queue_recovers_after_acknowledgement_and_reload() {
    let store = StateStore::new(BlockingAsync::new(Flash::new()), 0..65536).unwrap();
    let mut buffer = [0; 4096];
    let mut app = block_on(PersistentTraining::load(store, &mut buffer)).unwrap();
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 5, 0, 0).unwrap();
    for day in 0..=MAX_PENDING_RECORDS {
        block_on(app.advance(
            &mut buffer,
            start + Duration::days(day as i64),
            Some("Kata A"),
        ))
        .unwrap();
    }
    let now = start + Duration::days(MAX_PENDING_RECORDS as i64 + 1);
    let before = app.state().clone();
    assert!(matches!(
        block_on(app.advance(&mut buffer, now, Some("Kata B"))),
        Err(TransactionError::Training(TrainingError::QueueFull))
    ));
    assert_eq!(app.state(), &before);
    let oldest = app.state().pending()[0].date;
    block_on(app.acknowledge(&mut buffer, oldest)).unwrap();
    block_on(app.advance(&mut buffer, now, Some("Kata B"))).unwrap();
    let expected = app.state().clone();
    assert_eq!(expected.pending().len(), MAX_PENDING_RECORDS);
    assert_eq!(expected.active().unwrap().kata, "Kata B");
    let store = app.into_store();
    let reloaded = block_on(PersistentTraining::load(store, &mut buffer)).unwrap();
    assert_eq!(reloaded.state(), &expected);
}

#[test]
fn invalid_flash_ranges_are_rejected() {
    assert!(StateStore::new(BlockingAsync::new(Flash::new()), 8192..4096).is_err());
    assert!(StateStore::new(BlockingAsync::new(Flash::new()), 0..69632).is_err());
    assert!(StateStore::new(BlockingAsync::new(Flash::new()), 1..8192).is_err());
}
