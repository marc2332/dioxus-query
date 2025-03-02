use instant::Instant;
use std::{fmt::Debug, ops::Deref, time::Duration};

use crate::result::QueryState;

/// Cached result.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedResult<T, E> {
    pub(crate) value: QueryState<T, E>,
    pub(crate) instant: Option<Instant>,
    pub(crate) has_been_loaded: bool,
}

impl<T, E> CachedResult<T, E> {
    pub(crate) fn new(value: QueryState<T, E>) -> Self {
        Self {
            value,
            ..Default::default()
        }
    }

    /// Get this result's value
    pub fn value(&self) -> &QueryState<T, E> {
        &self.value
    }

    /// Set this result's value
    pub(crate) fn set_value(&mut self, new_value: QueryState<T, E>) {
        self.value = new_value;
        self.instant = Some(Instant::now());
    }

    /// Check if this result is stale yet
    pub fn is_fresh(&self, stale_time: Duration) -> bool {
        if let Some(instant) = self.instant {
            instant.elapsed() < stale_time
        } else {
            false
        }
    }

    /// Check if this result has loaded yet
    pub(crate) fn has_been_loaded(&self) -> bool {
        self.has_been_loaded
    }

    /// Set this result as loading
    pub(crate) fn set_to_loading(&mut self) {
        self.value.set_loading();
        self.instant = Some(Instant::now());
        self.has_been_loaded = true;
    }
}

impl<T, E> Deref for CachedResult<T, E> {
    type Target = QueryState<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, E> Default for CachedResult<T, E> {
    fn default() -> Self {
        Self {
            value: Default::default(),
            instant: None,
            has_been_loaded: false,
        }
    }
}
