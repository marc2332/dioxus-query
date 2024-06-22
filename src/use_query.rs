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
pub fn use_query<T, E, K, const N: usize>(
    query_keys: [K; N],
    query: impl FnOnce() -> Query<T, E, K>,
) -> UseQuery<T, E, K>
where
    T: 'static + PartialEq,
    E: 'static,
    K: 'static + Eq + Hash + Clone,
{
    let client = use_query_client();
    use_memo_with_old_value(
        query_keys.clone(),
        |prev_query: Option<UseQuery<T, E, K>>| {
            // If there is an previous query it means they query keys have changed.
            if let Some(prev_query) = prev_query {
                let prev_entry = &prev_query.cleaner.peek().registry_entry;
                let mut queries_registry = client.queries_registry.write_unchecked();
                // Remove the entry from the previous query
                queries_registry.remove(prev_entry).unwrap();
            }

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
        },
    )
}

/// Alternative to [use_memo]
/// Benefits:
/// - No unnecessary rerenders
/// - Access to the previous value when dependencies change
/// Downsides:
/// - T needs to be Clone (cannot be avoided)
fn use_memo_with_old_value<T: 'static + Clone, D: PartialEq + 'static>(
    deps: D,
    init: impl FnOnce(Option<T>) -> T,
) -> T {
    struct Memoized<T, D> {
        value: T,
        deps: D,
    }
    let mut value = use_signal::<Option<Memoized<T, D>>>(|| None);
    let mut memoized_value = value.write();

    let deps_have_changed = memoized_value
        .as_ref()
        .map(|memoized_value| &memoized_value.deps)
        != Some(&deps);

    let new_value = if deps_have_changed {
        let prev_value = memoized_value
            .take()
            .map(|memoized_value| memoized_value.value);
        Some(init(prev_value))
    } else {
        None
    };

    if let Some(new_value) = new_value {
        let new_memoized_value = Memoized {
            value: new_value,
            deps,
        };
        *memoized_value = Some(new_memoized_value);
    }

    memoized_value.as_ref().unwrap().value.clone()
}

/// Register a query listener with the given combination of **query keys** and **query function**.
/// See [UseQuery] on how to use it.
///
/// ## Example:
///
/// ```no_run
/// let users_query = use_get_query([QueryKey::User(id)], fetch_user);
/// ```
pub fn use_get_query<T, E, K, Q, F, const N: usize>(
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
