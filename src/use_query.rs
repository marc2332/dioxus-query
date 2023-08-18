use dioxus_core::*;

pub use futures_util;
use futures_util::future::BoxFuture;
use std::{
    any::TypeId,
    collections::HashSet,
    hash::Hash,
    sync::{Arc, RwLockReadGuard},
};

use crate::{
    cached_result::CachedResult,
    result::QueryResult,
    use_query_client::{
        use_query_client, QueryFn, QueryListeners, QueryValue, RegistryEntry, UseQueryClient,
    },
};

/// A query listener.
pub struct UseQuery<T, E, K: Eq + Hash> {
    client: UseQueryClient<T, E, K>,
    value: QueryValue<CachedResult<T, E>>,
    registry_entry: RegistryEntry<K>,
    scope_id: ScopeId,
}

impl<T, E, K: Eq + Hash> Drop for UseQuery<T, E, K> {
    fn drop(&mut self) {
        let is_empty = {
            let mut queries_registry = self.client.queries_registry.borrow_mut();
            let query_listeners = queries_registry.get_mut(&self.registry_entry).unwrap();
            // Remove this `UseQuery`'s listener
            query_listeners.listeners.remove(&self.scope_id);
            query_listeners.listeners.is_empty()
        };
        if is_empty {
            // Remove the query keys if this was the last listener listening
            self.client
                .queries_registry
                .borrow_mut()
                .remove(&self.registry_entry);
        }
    }
}

impl<T, E, K: Eq + Hash> UseQuery<T, E, K> {
    /// Get the current result from the query.
    pub fn result(&self) -> RwLockReadGuard<CachedResult<T, E>> {
        self.value.read().unwrap()
    }
}

/// The configuration for a given query listener.
pub struct QueryConfig<T, E, K> {
    query_fn: Arc<Box<QueryFn<T, E, K>>>,
    initial_fn: Option<Box<dyn Fn() -> QueryResult<T, E>>>,
    registry_entry: RegistryEntry<K>,
}

impl<T, E, K> QueryConfig<T, E, K> {
    pub fn new<F>(query_keys: Vec<K>, query_fn: F) -> Self
    where
        F: Fn(&[K]) -> BoxFuture<QueryResult<T, E>> + 'static + Send + Sync,
    {
        Self {
            query_fn: Arc::new(Box::new(query_fn)),
            initial_fn: None,
            registry_entry: RegistryEntry {
                query_keys,
                query_fn_id: TypeId::of::<F>(),
            },
        }
    }

    /// Set the initial value of the query.
    pub fn initial(mut self, initial_data: impl Fn() -> QueryResult<T, E> + 'static) -> Self {
        self.initial_fn = Some(Box::new(initial_data));
        self
    }
}

/// Register a query listener with the given configuration.
/// See [UseQuery] on how to use it.
pub fn use_query_config<T, E, K>(
    cx: &ScopeState,
    config: impl FnOnce() -> QueryConfig<T, E, K>,
) -> &UseQuery<T, E, K>
where
    T: 'static + PartialEq + Clone,
    E: 'static + PartialEq + Clone,
    K: Clone + Eq + Hash + 'static,
{
    let client = use_query_client(cx);
    let config = cx.use_hook(|| Arc::new(config()));

    cx.use_hook(|| {
        let mut queries_registry = client.queries_registry.borrow_mut();
        // Create a group of listeners for the given combination of keys
        let query_listeners = queries_registry
            .entry(config.registry_entry.clone())
            .or_insert(QueryListeners {
                listeners: HashSet::default(),
                value: QueryValue::default(),
                query_fn: config.query_fn.clone(),
            });
        // Register this component as listener of the keys combination
        query_listeners.listeners.insert(cx.scope_id());

        let entry = config.registry_entry.clone();

        // Initial async load
        cx.spawn({
            let client = client.clone();
            async move {
                client.validate_new_query(&entry).await;
            }
        });

        UseQuery {
            client: client.clone(),
            value: query_listeners.value.clone(),
            registry_entry: config.registry_entry.clone(),
            scope_id: cx.scope_id(),
        }
    })
}

/// Register a query listener with the given combination of **query keys** and **query function**.
/// See [UseQuery] on how to use it.
///
/// ## Example:
///
/// ```no_run
/// let users_query = use_query(cx, move || vec![QueryKeys::User(id)], fetch_user);
/// ```
pub fn use_query<T: Clone, E: Clone, K>(
    cx: &ScopeState,
    query_keys: impl FnOnce() -> Vec<K>,
    query_fn: impl Fn(&[K]) -> BoxFuture<QueryResult<T, E>> + 'static + Send + Sync,
) -> &UseQuery<T, E, K>
where
    T: 'static + PartialEq,
    E: 'static + PartialEq,
    K: Clone + Eq + Hash + 'static,
{
    use_query_config(cx, || QueryConfig::new(query_keys(), query_fn))
}
