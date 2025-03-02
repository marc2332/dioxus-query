use std::mem;

/// The result of a query.
pub type QueryResult<T, E> = Result<T, E>;

/// The state of a query.
#[derive(Clone, PartialEq, Debug)]
pub enum QueryState<T, E> {
    /// Contains a successful or errored result
    Settled(QueryResult<T, E>),
    /// Contains a loading state that may or not have a cached result
    Loading(Option<T>),
}

impl<T, E> QueryState<T, E> {
    pub fn is_ok(&self) -> bool {
        matches!(self, QueryState::Settled(Ok(..)))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, QueryState::Settled(Err(..)))
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, QueryState::Loading(..))
    }

    pub(crate) fn set_loading(&mut self) {
        let result = mem::replace(self, Self::Loading(None)).into();
        if let Some(v) = result {
            *self = Self::Loading(Some(v))
        }
    }
}

impl<T, E> Default for QueryState<T, E> {
    fn default() -> Self {
        Self::Loading(None)
    }
}

impl<T, E> From<QueryState<T, E>> for Option<T> {
    fn from(result: QueryState<T, E>) -> Self {
        match result {
            QueryState::Settled(Ok(v)) => Some(v),
            QueryState::Settled(Err(_)) => None,
            QueryState::Loading(v) => v,
        }
    }
}

impl<T, E> From<Result<T, E>> for QueryState<T, E> {
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(v) => QueryState::Settled(Ok(v)),
            Err(e) => QueryState::Settled(Err(e)),
        }
    }
}

impl<T, E> From<T> for QueryState<T, E> {
    fn from(value: T) -> Self {
        QueryState::Settled(Ok(value))
    }
}
