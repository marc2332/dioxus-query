use instant::Instant;
use std::{fmt::Debug, ops::Deref, time::Duration};

use crate::result::QueryState;

/// Cached result.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedResult<T, E> {
    pub(crate) state: QueryState<T, E>,
    pub(crate) instant: Option<Instant>,
    pub(crate) has_been_loaded: bool,
}

impl<T, E> CachedResult<T, E> {
    pub(crate) fn new(value: QueryState<T, E>) -> Self {
        Self {
            state: value,
            ..Default::default()
        }
    }

    /// Get this result's state
    pub fn state(&self) -> &QueryState<T, E> {
        &self.state
    }

    /// Set this result's value
    pub(crate) fn set_value(&mut self, new_value: QueryState<T, E>) {
        self.state = new_value;
        self.instant = Some(Instant::now());
    }

    /// Check if this result is stale yet
    pub fn is_stale(&self, stale_time: Duration) -> bool {
        if let Some(instant) = self.instant {
            instant.elapsed() > stale_time
        } else {
            true
        }
    }

    /// Check if this result has loaded yet
    pub(crate) fn has_been_loaded(&self) -> bool {
        self.has_been_loaded
    }

    /// Set this result as loading
    pub(crate) fn set_to_loading(&mut self) {
        self.state.set_loading();
        self.instant = Some(Instant::now());
    }

    /// Set this result as loaded
    pub(crate) fn set_to_loaded(&mut self) {
        self.has_been_loaded = true;
    }
}

impl<T, E> Deref for CachedResult<T, E> {
    type Target = QueryState<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<T, E> Default for CachedResult<T, E> {
    fn default() -> Self {
        Self {
            state: Default::default(),
            instant: None,
            has_been_loaded: false,
        }
    }
}
