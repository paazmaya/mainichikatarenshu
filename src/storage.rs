//! Experimental snapshot persistence in a reserved flash range.
//!
//! The underlying library can skip corrupt items or automatically repair flash;
//! arbitrary corruption and every power-loss point have not been validated.

use core::ops::Range;
use embedded_storage_async::nor_flash::NorFlash;
use sequential_storage::{
    cache::{Cache, Uncached},
    map::{MapConfig, MapConfigError, MapStorage},
};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::training::{Change, Error as TrainingError, Record, TrainingState};

const STATE_KEY: u8 = 1;
const STATE_VERSION: u8 = 1;

#[derive(Deserialize, Serialize)]
struct Snapshot {
    version: u8,
    state: TrainingState,
}

impl<'a> sequential_storage::map::PostcardValue<'a> for Snapshot {}

#[derive(Debug)]
pub enum StoreConfigError {
    InvalidRange,
    OutsideFlash,
    InvalidLayout(MapConfigError),
}

#[derive(Debug)]
pub enum StoreError<E> {
    Flash(sequential_storage::Error<E>),
    UnsupportedVersion(u8),
}

/// Flash-backed snapshot store. The caller must reserve and validate the range.
pub struct StateStore<F: NorFlash> {
    map: MapStorage<u8, F, Cache<Uncached, Uncached, Uncached, u8>>,
}

impl<F: NorFlash> StateStore<F> {
    pub fn new(flash: F, range: Range<u32>) -> Result<Self, StoreConfigError> {
        if range.start >= range.end {
            return Err(StoreConfigError::InvalidRange);
        }
        if range.end as usize > flash.capacity() {
            return Err(StoreConfigError::OutsideFlash);
        }
        let config = MapConfig::try_new(range).map_err(StoreConfigError::InvalidLayout)?;
        Ok(Self {
            map: MapStorage::new(flash, config, Cache::new_uncached()),
        })
    }
}

/// A state change is published only after the snapshot is written successfully.
pub struct PersistentTraining<F: NorFlash> {
    store: StateStore<F>,
    state: TrainingState,
    requires_reload: bool,
}

#[derive(Debug)]
pub enum TransactionError<E> {
    Training(TrainingError),
    Storage(StoreError<E>),
    RequiresReload,
}

impl<F: NorFlash> PersistentTraining<F> {
    pub async fn load(
        mut store: StateStore<F>,
        buffer: &mut [u8],
    ) -> Result<Self, StoreError<F::Error>> {
        let state = store.load(buffer).await?.unwrap_or_default();
        Ok(Self {
            store,
            state,
            requires_reload: false,
        })
    }

    pub fn state(&self) -> &TrainingState {
        &self.state
    }

    pub async fn advance(
        &mut self,
        buffer: &mut [u8],
        now: DateTime<Utc>,
        kata: Option<&str>,
    ) -> Result<Change, TransactionError<F::Error>> {
        self.check_ready()?;
        let mut next = self.state.clone();
        let change = next
            .advance(now, kata)
            .map_err(TransactionError::Training)?;
        if change != Change::None {
            self.save(buffer, &next).await?;
        }
        self.state = next;
        Ok(change)
    }

    pub async fn confirm(
        &mut self,
        buffer: &mut [u8],
        now: DateTime<Utc>,
    ) -> Result<Change, TransactionError<F::Error>> {
        self.check_ready()?;
        let mut next = self.state.clone();
        let change = next.confirm(now).map_err(TransactionError::Training)?;
        if change != Change::None {
            self.save(buffer, &next).await?;
        }
        self.state = next;
        Ok(change)
    }

    pub async fn acknowledge(
        &mut self,
        buffer: &mut [u8],
        date: NaiveDate,
    ) -> Result<Record, TransactionError<F::Error>> {
        self.check_ready()?;
        let mut next = self.state.clone();
        let record = next.acknowledge(date).map_err(TransactionError::Training)?;
        self.save(buffer, &next).await?;
        self.state = next;
        Ok(record)
    }

    pub fn into_store(self) -> StateStore<F> {
        self.store
    }

    fn check_ready(&self) -> Result<(), TransactionError<F::Error>> {
        if self.requires_reload {
            Err(TransactionError::RequiresReload)
        } else {
            Ok(())
        }
    }

    async fn save(
        &mut self,
        buffer: &mut [u8],
        next: &TrainingState,
    ) -> Result<(), TransactionError<F::Error>> {
        if let Err(error) = self.store.save(buffer, next).await {
            self.requires_reload = true;
            return Err(TransactionError::Storage(error));
        }
        Ok(())
    }
}

impl<F: NorFlash> StateStore<F> {
    /// Return the latest snapshot found by the underlying map, or `None`.
    /// Some corrupt items can be skipped by the map rather than reported.
    pub async fn load(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<Option<TrainingState>, StoreError<F::Error>> {
        let snapshot: Option<Snapshot> = self
            .map
            .fetch_item(buffer, &STATE_KEY)
            .await
            .map_err(StoreError::Flash)?;
        match snapshot {
            None => Ok(None),
            Some(snapshot) if snapshot.version == STATE_VERSION => Ok(Some(snapshot.state)),
            Some(snapshot) => Err(StoreError::UnsupportedVersion(snapshot.version)),
        }
    }

    /// Store the complete snapshot. A failed write must be treated as ambiguous.
    pub async fn save(
        &mut self,
        buffer: &mut [u8],
        state: &TrainingState,
    ) -> Result<(), StoreError<F::Error>> {
        let snapshot = Snapshot {
            version: STATE_VERSION,
            state: state.clone(),
        };
        self.map
            .store_item(buffer, &STATE_KEY, &snapshot)
            .await
            .map_err(StoreError::Flash)
    }

    pub fn into_flash(self) -> F {
        self.map.destroy().0
    }
}
