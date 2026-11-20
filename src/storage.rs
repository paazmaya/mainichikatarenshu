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
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::network_config::Credentials;
use crate::training::{Change, Error as TrainingError, Record, TrainingState};

const STATE_KEY: u8 = 1;
const STATE_VERSION: u8 = 1;
const WIFI_CREDENTIALS_KEY: u8 = 2;
const WIFI_CREDENTIALS_VERSION: u8 = 1;

/// A stored value prefixed with its format version.
///
/// Postcard does not encode field names, so this is byte-compatible with the
/// earlier per-type `{ version, state }` and `{ version, credentials }` snapshots.
#[derive(Deserialize, Serialize)]
struct Versioned<T> {
    version: u8,
    value: T,
}

impl<'a, T: Serialize + Deserialize<'a>> sequential_storage::map::PostcardValue<'a>
    for Versioned<T>
{
}

/// The flash range cannot hold the store.
#[derive(Debug)]
pub enum StoreConfigError {
    /// The range is empty or reversed.
    InvalidRange,
    /// The range extends past the end of the flash.
    OutsideFlash,
    /// The range does not fit the map's sector layout.
    InvalidLayout(MapConfigError),
}

/// Reading or writing a stored item failed.
#[derive(Debug)]
pub enum StoreError<E> {
    /// The flash or the map reported an error.
    Flash(sequential_storage::Error<E>),
    /// The item was written by an unsupported format version.
    UnsupportedVersion(u8),
}

/// Flash-backed snapshot store. The caller must reserve and validate the range.
pub struct StateStore<F: NorFlash> {
    map: MapStorage<u8, F, Cache<Uncached, Uncached, Uncached, u8>>,
}

impl<F: NorFlash> StateStore<F> {
    /// Use `range` of `flash` for storage; see [`crate::partitions::find_training_partition`].
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

    /// Return the latest training snapshot found by the underlying map, or `None`.
    /// Some corrupt items can be skipped by the map rather than reported.
    pub async fn load(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<Option<TrainingState>, StoreError<F::Error>> {
        self.fetch_versioned(buffer, STATE_KEY, STATE_VERSION).await
    }

    /// Store the complete training snapshot. A failed write must be treated as ambiguous.
    pub async fn save(
        &mut self,
        buffer: &mut [u8],
        state: &TrainingState,
    ) -> Result<(), StoreError<F::Error>> {
        self.store_versioned(buffer, STATE_KEY, STATE_VERSION, state)
            .await
    }

    /// Load stored Wi-Fi credentials, or `None` if not stored.
    pub async fn load_wifi_credentials(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<Option<Credentials>, StoreError<F::Error>> {
        self.fetch_versioned(buffer, WIFI_CREDENTIALS_KEY, WIFI_CREDENTIALS_VERSION)
            .await
    }

    /// Store Wi-Fi credentials to flash.
    pub async fn save_wifi_credentials(
        &mut self,
        buffer: &mut [u8],
        credentials: &Credentials,
    ) -> Result<(), StoreError<F::Error>> {
        self.store_versioned(
            buffer,
            WIFI_CREDENTIALS_KEY,
            WIFI_CREDENTIALS_VERSION,
            credentials,
        )
        .await
    }

    /// Release the underlying flash.
    pub fn into_flash(self) -> F {
        self.map.destroy().0
    }

    async fn fetch_versioned<T: Serialize + DeserializeOwned>(
        &mut self,
        buffer: &mut [u8],
        key: u8,
        version: u8,
    ) -> Result<Option<T>, StoreError<F::Error>> {
        let item: Option<Versioned<T>> = self
            .map
            .fetch_item(buffer, &key)
            .await
            .map_err(StoreError::Flash)?;
        match item {
            None => Ok(None),
            Some(item) if item.version == version => Ok(Some(item.value)),
            Some(item) => Err(StoreError::UnsupportedVersion(item.version)),
        }
    }

    async fn store_versioned<T: Serialize + DeserializeOwned + Clone>(
        &mut self,
        buffer: &mut [u8],
        key: u8,
        version: u8,
        value: &T,
    ) -> Result<(), StoreError<F::Error>> {
        let item = Versioned {
            version,
            value: value.clone(),
        };
        self.map
            .store_item(buffer, &key, &item)
            .await
            .map_err(StoreError::Flash)
    }
}

/// Training state backed by a [`StateStore`].
///
/// A state change is published only after the snapshot is written
/// successfully. After a failed write the flash content is ambiguous, so all
/// further transactions fail with [`TransactionError::RequiresReload`].
pub struct PersistentTraining<F: NorFlash> {
    store: StateStore<F>,
    state: TrainingState,
    requires_reload: bool,
}

/// A training transaction was not applied.
#[derive(Debug)]
pub enum TransactionError<E> {
    /// The state machine rejected the request; nothing was written.
    Training(TrainingError),
    /// Writing the snapshot failed; the stored state is ambiguous.
    Storage(StoreError<E>),
    /// An earlier write failed; reload from flash before continuing.
    RequiresReload,
}

impl<F: NorFlash> PersistentTraining<F> {
    /// Load the latest snapshot, or start from an empty state.
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

    /// The last successfully persisted state.
    pub fn state(&self) -> &TrainingState {
        &self.state
    }

    /// Persist [`TrainingState::advance`].
    pub async fn advance(
        &mut self,
        buffer: &mut [u8],
        now: DateTime<Utc>,
        kata: Option<&str>,
    ) -> Result<Change, TransactionError<F::Error>> {
        self.transact(
            buffer,
            |state| state.advance(now, kata),
            |change| *change != Change::None,
        )
        .await
    }

    /// Persist [`TrainingState::confirm`].
    pub async fn confirm(
        &mut self,
        buffer: &mut [u8],
        now: DateTime<Utc>,
    ) -> Result<Change, TransactionError<F::Error>> {
        self.transact(
            buffer,
            |state| state.confirm(now),
            |change| *change != Change::None,
        )
        .await
    }

    /// Persist [`TrainingState::acknowledge`].
    pub async fn acknowledge(
        &mut self,
        buffer: &mut [u8],
        date: NaiveDate,
    ) -> Result<Record, TransactionError<F::Error>> {
        self.transact(buffer, |state| state.acknowledge(date), |_| true)
            .await
    }

    /// Load stored Wi-Fi credentials from the underlying store.
    pub async fn load_wifi_credentials(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<Option<Credentials>, StoreError<F::Error>> {
        self.store.load_wifi_credentials(buffer).await
    }

    /// Save Wi-Fi credentials to the underlying store.
    pub async fn save_wifi_credentials(
        &mut self,
        buffer: &mut [u8],
        credentials: &Credentials,
    ) -> Result<(), StoreError<F::Error>> {
        self.store.save_wifi_credentials(buffer, credentials).await
    }

    /// Release the underlying store.
    pub fn into_store(self) -> StateStore<F> {
        self.store
    }

    /// Apply `update` to a copy of the state, persist it if `needs_save`, then publish it.
    async fn transact<T>(
        &mut self,
        buffer: &mut [u8],
        update: impl FnOnce(&mut TrainingState) -> Result<T, TrainingError>,
        needs_save: impl FnOnce(&T) -> bool,
    ) -> Result<T, TransactionError<F::Error>> {
        if self.requires_reload {
            return Err(TransactionError::RequiresReload);
        }
        let mut next = self.state.clone();
        let outcome = update(&mut next).map_err(TransactionError::Training)?;
        if needs_save(&outcome) {
            if let Err(error) = self.store.save(buffer, &next).await {
                self.requires_reload = true;
                return Err(TransactionError::Storage(error));
            }
        }
        self.state = next;
        Ok(outcome)
    }
}
