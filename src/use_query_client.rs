use dioxus::prelude::*;
use futures_util::{
    stream::{FuturesUnordered, StreamExt},
    Future,
};
use instant::Instant;
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
    sync::{Arc, RwLock},
};

use crate::{cached_result::CachedResult, result::QueryResult};

pub fn use_init_query_client<T, E, K>(cx: &ScopeState) -> &UseQueryClient<T, E, K>
where
    T: 'static + Clone,
    E: 'static + Clone,
    K: 'static + Clone,
{
    use_context_provider(cx, || UseQueryClient {
        queries_registry: Rc::default(),
        scheduler: cx.schedule_update_any(),
    })
}

/// Get access to the [UseQueryClient].
pub fn use_query_client<T, E, K>(cx: &ScopeState) -> &UseQueryClient<T, E, K>
where
    T: 'static + Clone,
    E: 'static + Clone,
    K: 'static + Clone,
{
    use_context(cx).unwrap()
}

pub(crate) type QueryFn<T, E, K> = dyn Fn(Vec<K>) -> Box<dyn Future<Output = QueryResult<T, E>>>;

pub(crate) type QueryValue<T> = Arc<RwLock<T>>;

#[derive(Clone)]
pub(crate) struct QueryListeners<T, E, K> {
    pub(crate) value: QueryValue<CachedResult<T, E>>,
    pub(crate) listeners: HashSet<ScopeId>,
    pub(crate) query_fn: Arc<Box<QueryFn<T, E, K>>>,
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
#[derive(Clone)]
pub struct UseQueryClient<T, E, K> {
    pub(crate) queries_registry: Rc<RefCell<QueriesRegistry<T, E, K>>>,
    pub(crate) scheduler: Arc<dyn Fn(ScopeId)>,
}

impl<T, E, K> UseQueryClient<T, E, K>
where
    T: 'static + Clone,
    E: 'static + Clone,
    K: 'static + PartialEq + Eq + Hash + Clone,
{
    pub(crate) fn get_entry(&self, entry: &RegistryEntry<K>) -> QueryListeners<T, E, K> {
        let registry = self.queries_registry.borrow();
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
            // Only change to `Loading` if had been changed at some point
            if has_been_mutated {
                let cached_value: Option<T> = value.read().unwrap().clone().into();
                *value.write().unwrap() = CachedResult {
                    value: QueryResult::Loading(cached_value),
                    instant: Some(Instant::now()),
                    has_been_queried: true,
                };
                for listener in listeners {
                    (self.scheduler)(listener);
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
                (self.scheduler)(listener);
            }
        } else {
            for listener in listeners {
                (self.scheduler)(listener);
            }
        }
    }

    pub(crate) async fn invalidate_queries_inner(
        queries_registry: Rc<RefCell<QueriesRegistry<T, E, K>>>,
        scheduler: Arc<dyn Fn(ScopeId)>,
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
        ) in queries_registry.borrow().iter()
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
                // Only change to `Loading` if had been changed at some point
                let cached_value: Option<T> = value.read().unwrap().clone().into();
                *value.write().unwrap() = CachedResult {
                    value: QueryResult::Loading(cached_value),
                    instant: Some(Instant::now()),
                    has_been_queried: true,
                };
                for listener in &query_listeners {
                    (scheduler)(*listener);
                }

                to_owned![query_fn, query_keys, query_listeners, value, scheduler];

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
                        scheduler(listener);
                    }
                }));
            }
        }

        tasks.count().await;
    }

    /// Invalidate a single query.
    /// It will run alone, after previous queries have finished.
    pub fn invalidate_query(&self, key_to_invalidate: K) {
        let queries_registry = self.queries_registry.clone();
        let scheduler = self.scheduler.clone();
        dioxus::prelude::spawn(async move {
            Self::invalidate_queries_inner(queries_registry, scheduler, &[key_to_invalidate]).await;
        });
    }

    /// Invalidate a group of queries.
    /// They will all run concurrently, after previous queries have finished.
    pub fn invalidate_queries(&self, keys_to_invalidate: &[K]) {
        let queries_registry = self.queries_registry.clone();
        let scheduler = self.scheduler.clone();
        let keys_to_invalidate = keys_to_invalidate.to_vec();
        dioxus::prelude::spawn(async move {
            Self::invalidate_queries_inner(queries_registry, scheduler, &keys_to_invalidate).await;
        });
    }
}
