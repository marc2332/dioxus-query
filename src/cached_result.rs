use instant::Instant;
use std::{fmt::Debug, ops::Deref, time::Duration};

use crate::result::QueryResult;

const STALE_TIME: u64 = 100;

/// Cached result.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedResult<T, E> {
    pub(crate) value: QueryResult<T, E>,
    pub(crate) instant: Option<Instant>,
    pub(crate) has_been_queried: bool,
}

impl<T, E> CachedResult<T, E> {
    pub fn new(value: QueryResult<T, E>) -> Self {
        Self {
            value,
            ..Default::default()
        }
    }

    /// Get this result's value
    pub fn value(&self) -> &QueryResult<T, E> {
        &self.value
    }

    /// Check if this result has been mutated recently
    pub fn is_fresh(&self) -> bool {
        if let Some(instant) = self.instant {
            instant.elapsed().as_millis() < Duration::from_millis(STALE_TIME).as_millis()
        } else {
            false
        }
    }

    /// Check if this result has been mutated at some point
    pub(crate) fn has_been_mutated(&self) -> bool {
        self.instant.is_some()
    }

    /// Check if this result has queried it's actual value yet
    pub(crate) fn has_been_queried(&self) -> bool {
        self.has_been_queried
    }

    pub(crate) fn set_to_loading(&mut self) {
        self.value.set_loading();
        self.instant = Some(Instant::now());
        self.has_been_queried = true;
    }
}

impl<T, E> Deref for CachedResult<T, E> {
    type Target = QueryResult<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T, E> Default for CachedResult<T, E> {
    fn default() -> Self {
        Self {
            value: Default::default(),
            instant: None,
            has_been_queried: false,
        }
    }
}
