use dioxus_lib::prelude::*;
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
pub struct UseQuery<T, E, K>
where
    T: 'static,
    E: 'static,
    K: 'static + Eq + Hash,
{
    cleaner: Signal<UseQueryCleaner<T, E, K>>,
    client: UseQueryClient<T, E, K>,
    value: QueryValue<CachedResult<T, E>>,
    scope_id: ScopeId,
}

impl<T, E, K> Clone for UseQuery<T, E, K>
where
    K: Eq + Hash + Clone,
{
    fn clone(&self) -> Self {
        Self {
            cleaner: self.cleaner,
            client: self.client,
            value: self.value.clone(),
            scope_id: self.scope_id,
        }
    }
}

impl<T, E, K: Eq + Hash> UseQuery<T, E, K> {
    /// Get the current result from the query.
    pub fn result(&self) -> RwLockReadGuard<CachedResult<T, E>> {
        self.value.read().expect("Query value is already borrowed")
    }
}

pub struct UseQueryCleaner<T, E, K>
where
    T: 'static,
    E: 'static,
    K: 'static + Eq + Hash,
{
    client: UseQueryClient<T, E, K>,
    registry_entry: RegistryEntry<K>,
    scope_id: ScopeId,
}

impl<T, E, K: Eq + Hash> Drop for UseQueryCleaner<T, E, K> {
    fn drop(&mut self) {
        let was_last_listener = {
            let mut queries_registry = self.client.queries_registry.write_unchecked();
            let query_listeners = queries_registry.get_mut(&self.registry_entry).unwrap();
            // Remove this listener
            query_listeners.listeners.remove(&self.scope_id);
            query_listeners.listeners.is_empty()
        };

        // Clear the queries registry of this listener if it was the last one
        if was_last_listener {
            self.client
                .queries_registry
                .write_unchecked()
                .remove(&self.registry_entry);
        }
    }
}

/// The configuration for a given query listener.
pub struct Query<T, E, K> {
    query_fn: Arc<Box<QueryFn<T, E, K>>>,
    initial_value: Option<QueryResult<T, E>>,
    registry_entry: RegistryEntry<K>,
}

impl<T, E, K> Query<T, E, K> {
    pub fn new<Q, F>(query_fn: Q) -> Self
    where
        Q: 'static + Fn(Vec<K>) -> F,
        F: 'static + Future<Output = QueryResult<T, E>>,
        K: Clone,
    {
        Self {
            query_fn: Arc::new(Box::new(move |q| {
                let fut = query_fn(q);
                Box::new(fut)
            })),
            initial_value: None,
            registry_entry: RegistryEntry {
                query_keys: Vec::new(),
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

/// Register a query listener with the given query configuration.
/// See [UseQuery] on how to use it.
pub fn use_query<T, E, K, const N: usize>(query_keys: [K; N], query: impl FnOnce() -> Query<T, E, K>) -> UseQuery<T, E, K>
where
    T: 'static + PartialEq,
    E: 'static,
    K: 'static + Eq + Hash + Clone,
{
    let client = use_query_client();
    let mut used_query = use_hook(|| {
        let mut query = query();
        query.registry_entry.query_keys = query_keys.to_vec();
        let registry_entry = query.registry_entry;
        let mut queries_registry = client.queries_registry.write_unchecked();

        // Create a group of listeners for the given [RegistryEntry] key.
        let query_listeners =
            queries_registry
                .entry(registry_entry.clone())
                .or_insert(QueryListeners {
                    listeners: HashSet::default(),
                    value: QueryValue::new(RwLock::new(CachedResult::new(
                        query.initial_value.unwrap_or_default(),
                    ))),
                    query_fn: query.query_fn,
                });

        // Register this listener's scope
        query_listeners
            .listeners
            .insert(current_scope_id().unwrap());

        let value = query_listeners.value.clone();

        // Asynchronously initialize the query value
        spawn({
            to_owned![registry_entry];
            async move {
                client.run_new_query(&registry_entry).await;
            }
        });

        UseQuery {
            client,
            value,
            scope_id: current_scope_id().unwrap(),
            cleaner: Signal::new(UseQueryCleaner {
                client,
                registry_entry,
                scope_id: current_scope_id().unwrap(),
            }),
        }
    });

    // Query keys have changed
    let used_entry = used_query.cleaner.peek().registry_entry.clone();

    if used_entry.query_keys != query_keys {
        let mut queries_registry = client.queries_registry.write_unchecked();

        // Remove the old entry
        let old_value = queries_registry.get(&used_entry).unwrap().clone();

        let new_entry  = RegistryEntry {
            query_keys: query_keys.to_vec(),
            ..used_entry
        };

        // Create a group of listeners for the given [RegistryEntry] key.
        let query_listeners =
            queries_registry
                .entry(new_entry.clone())
                .or_insert(QueryListeners {
                    listeners: HashSet::default(),
                    value: old_value.value,
                    query_fn: old_value.query_fn,
                });

        // Register this listener's scope
        query_listeners
            .listeners
            .insert(current_scope_id().unwrap());

        let _ = drop(queries_registry);

        // Replace the query cleaner with the a new entry
        *used_query.cleaner.write() = UseQueryCleaner {
            client,
            registry_entry: new_entry,
            scope_id: current_scope_id().unwrap(),
        };
    }

    used_query
}

/// Register a query listener with the given combination of **query keys** and **query function**.
/// See [UseQuery] on how to use it.
///
/// ## Example:
///
/// ```no_run
/// let users_query = use_simple_query([QueryKeys::User(id)], fetch_user);
/// ```
pub fn use_simple_query<T, E, K, Q, F, const N: usize>(
    query_keys: [K; N],
    query_fn: Q,
) -> UseQuery<T, E, K>
where
    T: 'static + PartialEq,
    E: 'static,
    K: 'static + Eq + Hash + Clone,
    Q: 'static + Fn(Vec<K>) -> F,
    F: 'static + Future<Output = QueryResult<T, E>>,
{
    use_query(query_keys, || Query::new(query_fn))
}
