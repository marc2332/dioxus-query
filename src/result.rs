use std::mem;

/// The result of a query.
#[derive(Clone, PartialEq, Debug)]
pub enum QueryResult<T, E> {
    /// Contains a successful state
    Ok(T),
    /// Contains an errored state
    Err(E),
    /// Contains a loading state that may or not have a cached result
    Loading(Option<T>),
}

impl<T, E> QueryResult<T, E> {
    pub fn is_ok(&self) -> bool {
        matches!(self, QueryResult::Ok(..))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, QueryResult::Err(..))
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, QueryResult::Loading(..))
    }

    pub fn set_loading(&mut self) {
        let result = mem::replace(self, Self::Loading(None)).into();
        if let Some(v) = result {
            *self = Self::Loading(Some(v))
        }
    }
}

impl<T, E> Default for QueryResult<T, E> {
    fn default() -> Self {
        Self::Loading(None)
    }
}

impl<T, E> From<QueryResult<T, E>> for Option<T> {
    fn from(result: QueryResult<T, E>) -> Self {
        match result {
            QueryResult::Ok(v) => Some(v),
            QueryResult::Err(_) => None,
            QueryResult::Loading(v) => v,
        }
    }
}

impl<T, E> From<Result<T, E>> for QueryResult<T, E> {
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(v) => QueryResult::Ok(v),
            Err(e) => QueryResult::Err(e),
        }
    }
}

impl<T, E> From<T> for QueryResult<T, E> {
    fn from(value: T) -> Self {
        QueryResult::Ok(value)
    }
}
