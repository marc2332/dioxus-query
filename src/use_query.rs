use dioxus_core::*;
use dioxus_hooks::to_owned;
use futures_util::Future;
use std::{
    any::TypeId,
    collections::HashSet,
    hash::Hash,
    sync::{Arc, RwLock, RwLockReadGuard},
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

impl<T, E, K: Eq + Hash> UseQuery<T, E, K> {
    /// Get the current result from the query.
    pub fn result(&self) -> RwLockReadGuard<CachedResult<T, E>> {
        self.value.read().expect("Query value is already borrowed")
    }
}

impl<T, E, K: Eq + Hash> Drop for UseQuery<T, E, K> {
    fn drop(&mut self) {
        let was_last_listener = {
            let mut queries_registry = self.client.queries_registry.borrow_mut();
            let query_listeners = queries_registry.get_mut(&self.registry_entry).unwrap();
            // Remove this listener
            query_listeners.listeners.remove(&self.scope_id);
            query_listeners.listeners.is_empty()
        };

        // Clear the queries registry of this listener if it was the last one
        if was_last_listener {
            self.client
                .queries_registry
                .borrow_mut()
                .remove(&self.registry_entry);
        }
    }
}

/// The configuration for a given query listener.
pub struct QueryConfig<T, E, K> {
    query_fn: Arc<Box<QueryFn<T, E, K>>>,
    initial_value: Option<QueryResult<T, E>>,
    registry_entry: RegistryEntry<K>,
}

impl<T, E, K> QueryConfig<T, E, K> {
    pub fn new<Q, F>(query_keys: Vec<K>, query_fn: Q) -> Self
    where
        Q: 'static + Fn(Vec<K>) -> F,
        F: 'static + Future<Output = QueryResult<T, E>>,
    {
        Self {
            query_fn: Arc::new(Box::new(move |q| {
                let fut = query_fn(q);
                Box::new(fut)
            })),
            initial_value: None,
            registry_entry: RegistryEntry {
                query_keys,
                query_fn_id: TypeId::of::<F>(),
            },
        }
    }

    /// Set the initial value of the query.
    pub fn initial(mut self, initial_value: QueryResult<T, E>) -> Self {
        self.initial_value = Some(initial_value);
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
    K: 'static + Eq + Hash + Clone,
{
    let client = use_query_client(cx);
    cx.use_hook(|| {
        let config = config();
        let registry_entry = config.registry_entry.clone();
        let mut queries_registry = client.queries_registry.borrow_mut();

        // Create a group of listeners for the given [RegistryEntry] key.
        let query_listeners =
            queries_registry
                .entry(registry_entry.clone())
                .or_insert(QueryListeners {
                    listeners: HashSet::default(),
                    value: QueryValue::new(RwLock::new(CachedResult::new(
                        config.initial_value.unwrap_or_default(),
                    ))),
                    query_fn: config.query_fn.clone(),
                });

        // Register this listener's scope
        query_listeners.listeners.insert(cx.scope_id());

        // Asynchronously initialize the query value
        cx.spawn({
            to_owned![client, registry_entry];
            async move {
                client.run_new_query(&registry_entry).await;
            }
        });

        UseQuery {
            client: client.clone(),
            value: query_listeners.value.clone(),
            registry_entry: registry_entry.clone(),
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
/// let users_query = use_query(cx, || vec![QueryKeys::User(id)], fetch_user);
/// ```
pub fn use_query<T, E, K, Q, F>(
    cx: &ScopeState,
    query_keys: impl FnOnce() -> Vec<K>,
    query_fn: Q,
) -> &UseQuery<T, E, K>
where
    T: 'static + PartialEq + Clone,
    E: 'static + PartialEq + Clone,
    K: 'static + Eq + Hash + Clone,
    Q: 'static + Fn(Vec<K>) -> F,
    F: 'static + Future<Output = QueryResult<T, E>>,
{
    use_query_config(cx, || QueryConfig::new(query_keys(), query_fn))
}
