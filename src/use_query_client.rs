use dioxus_lib::prelude::*;
use futures_util::{
    stream::{FuturesUnordered, StreamExt},
    Future,
};
use instant::Instant;
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::{Arc, RwLock},
};

use crate::{cached_result::CachedResult, result::QueryResult};

pub fn use_init_query_client<T, E, K>() -> UseQueryClient<T, E, K>
where
    T: 'static,
    E: 'static,
    K: 'static,
{
    use_context_provider(|| UseQueryClient {
        queries_registry: Signal::default(),
        scheduler: Signal::new(schedule_update_any()),
    })
}

/// Get access to the [UseQueryClient].
pub fn use_query_client<T, E, K>() -> UseQueryClient<T, E, K>
where
    T: 'static,
    E: 'static,
    K: 'static,
{
    use_context()
}

pub(crate) type QueryFn<T, E, K> = dyn Fn(Vec<K>) -> Box<dyn Future<Output = QueryResult<T, E>>>;

pub(crate) type QueryValue<T> = Arc<RwLock<T>>;

pub(crate) struct QueryListeners<T, E, K> {
    pub(crate) value: QueryValue<CachedResult<T, E>>,
    pub(crate) listeners: HashSet<ScopeId>,
    pub(crate) query_fn: Arc<Box<QueryFn<T, E, K>>>,
}

impl<T, E, K> Clone for QueryListeners<T, E, K> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            listeners: self.listeners.clone(),
            query_fn: self.query_fn.clone(),
        }
    }
}

/// Query listeners are grouped by their query keys and query functions
/// to avoid requesting the same data multiple times
#[derive(PartialEq, Eq, Hash, Clone)]
pub(crate) struct RegistryEntry<K> {
    pub(crate) query_keys: Vec<K>,
    pub(crate) query_fn_id: TypeId,
}

pub(crate) type QueriesRegistry<T, E, K> = HashMap<RegistryEntry<K>, QueryListeners<T, E, K>>;

/// Manage the queries of your application.
pub struct UseQueryClient<T, E, K>
where
    T: 'static,
    E: 'static,
    K: 'static,
{
    pub(crate) queries_registry: Signal<QueriesRegistry<T, E, K>>,
    pub(crate) scheduler: Signal<Arc<dyn Fn(ScopeId)>>,
}

impl<T, E, K> Clone for UseQueryClient<T, E, K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, E, K> Copy for UseQueryClient<T, E, K> {}

impl<T, E, K> UseQueryClient<T, E, K>
where
    T: 'static,
    E: 'static,
    K: 'static + PartialEq + Eq + Hash + Clone,
{
    pub(crate) fn get_entry(&self, entry: &RegistryEntry<K>) -> QueryListeners<T, E, K> {
        let registry = self.queries_registry.write_unchecked();
        registry.get(entry).unwrap().clone()
    }

    pub(crate) async fn run_new_query(&self, entry: &RegistryEntry<K>) {
        let QueryListeners {
            value,
            query_fn,
            listeners,
            ..
        } = self.get_entry(entry);

        let is_fresh = value.read().unwrap().is_fresh();
        let is_loading = value.read().unwrap().is_loading();
        let has_been_mutated = value.read().unwrap().has_been_mutated();
        let has_been_queried = value.read().unwrap().has_been_queried();

        if (!is_fresh && !is_loading) || !has_been_queried {
            // Only change to `Loading` if it has been changed at some point
            if has_been_mutated {
                value.write().unwrap().set_to_loading();
                for listener in listeners {
                    (self.scheduler.peek())(listener);
                }
            }

            // Mark as queried
            value.write().unwrap().has_been_queried = true;

            // Fetch the result
            let fut = (query_fn)(entry.query_keys.clone());
            let fut = Box::into_pin(fut);
            let new_value = fut.await;
            *value.write().unwrap() = CachedResult {
                value: new_value,
                instant: Some(Instant::now()),
                has_been_queried: true,
            };

            // Get the listeners again in case they changed
            let QueryListeners { listeners, .. } = self.get_entry(entry);

            for listener in listeners {
                (self.scheduler.peek())(listener);
            }
        } else {
            for listener in listeners {
                (self.scheduler.peek())(listener);
            }
        }
    }

    pub(crate) async fn invalidate_queries_inner(
        queries_registry: Signal<QueriesRegistry<T, E, K>>,
        scheduler: Signal<Arc<dyn Fn(ScopeId)>>,
        keys_to_invalidate: &[K],
    ) {
        let tasks = FuturesUnordered::new();
        for (
            RegistryEntry { query_keys, .. },
            QueryListeners {
                value,
                listeners,
                query_fn,
            },
        ) in queries_registry.peek().iter()
        {
            let mut query_listeners = HashSet::<ScopeId>::default();

            // Add the listeners of this `query_keys` when at least one of the keys match
            if query_keys.iter().any(|k| keys_to_invalidate.contains(k)) {
                for listener in listeners {
                    query_listeners.insert(*listener);
                }
            }

            // Save the group of listeners
            if !query_listeners.is_empty() {
                // Only change to `Loading` if it has been changed at some point
                value.write().unwrap().set_to_loading();
                for listener in &query_listeners {
                    (scheduler.peek())(*listener);
                }

                to_owned![query_fn, query_keys, query_listeners, value];

                tasks.push(Box::pin(async move {
                    // Fetch the result
                    let fut = (query_fn)(query_keys.clone());
                    let fut = Box::into_pin(fut);
                    let new_value = fut.await;
                    *value.write().unwrap() = CachedResult {
                        value: new_value,
                        instant: Some(Instant::now()),
                        has_been_queried: true,
                    };

                    for listener in query_listeners {
                        (scheduler.peek())(listener);
                    }
                }));
            }
        }

        tasks.count().await;
    }

    /// Invalidate a single query.
    /// It will run alone, after previous queries have finished.
    pub fn invalidate_query(&self, key_to_invalidate: K) {
        let queries_registry = self.queries_registry;
        let scheduler = self.scheduler;
        spawn(async move {
            Self::invalidate_queries_inner(queries_registry, scheduler, &[key_to_invalidate]).await;
        });
    }

    /// Invalidate a group of queries.
    /// They will all run concurrently, after previous queries have finished.
    pub fn invalidate_queries(&self, keys_to_invalidate: &[K]) {
        let queries_registry = self.queries_registry;
        let scheduler = self.scheduler;
        let keys_to_invalidate = keys_to_invalidate.to_vec();
        spawn(async move {
            Self::invalidate_queries_inner(queries_registry, scheduler, &keys_to_invalidate).await;
        });
    }
}
