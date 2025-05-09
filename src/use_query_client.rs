use dioxus_lib::prelude::*;
use futures_util::{
    stream::{FuturesUnordered, StreamExt},
    Future,
};
use std::{
    any::TypeId,
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use crate::{
    cached_result::{CachedResult, RevalidationOptions},
    result::QueryResult,
};

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

pub(crate) type QueryValue<T> = Rc<RefCell<T>>;

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

    pub(crate) async fn run_new_query(
        &self,
        entry: &RegistryEntry<K>,
        stale_time: Duration,
        revalidate_interval: Option<Duration>,
    ) {
        let QueryListeners {
            value,
            query_fn,
            listeners,
        } = self.get_entry(entry);

        let is_fresh = value.borrow().is_fresh(stale_time);
        let is_loading = value.borrow().is_loading();
        let has_been_queried = value.borrow().has_been_loaded();

        if (!is_fresh && !is_loading) || !has_been_queried {
            // If the query still has its initial state because it hasn't been loaded yet
            // we don't need to mark the value as loading, it would be an unnecesssary notification.
            if has_been_queried {
                value.borrow_mut().set_to_loading();
                for listener in listeners {
                    (self.scheduler.peek())(listener);
                }
            }

            // Run the query function
            let fut = (query_fn)(entry.query_keys.clone());
            let fut = Box::into_pin(fut);
            let new_value = fut.await;
            value.borrow_mut().set_value(new_value.into());

            // Get the listeners again in case they changed while the query function was running
            let QueryListeners { listeners, .. } = self.get_entry(entry);
            for listener in listeners {
                (self.scheduler.peek())(listener);
            }
        }
        // Handle revalidation if we receive an interval that is different from the current one.
        if value
            .borrow()
            .revalidation_options
            .as_ref()
            .map(|o| o.interval)
            != revalidate_interval
        {
            // Drop the existing task, if it exists.
            if let Some(task) = value.borrow().revalidation_options.as_ref().map(|o| o.task) {
                task.cancel();
            }
            let new_revalidate_options = match revalidate_interval {
                // When we do have an interval to update with, we set up a task to revalidate the query.
                Some(interval) => {
                    let query_keys = entry.query_keys.clone();
                    let self_clone = self.clone();
                    let task = spawn(async move {
                        loop {
                            // Wait for the specified interval, using the appropriate sleep function.
                            #[cfg(not(target_family = "wasm"))]
                            tokio::time::sleep(interval).await;
                            #[cfg(target_family = "wasm")]
                            wasmtimer::tokio::sleep(interval).await;

                            self_clone.invalidate_queries(&query_keys);
                        }
                    });
                    Some(RevalidationOptions {
                        interval: revalidate_interval.unwrap(),
                        task,
                    })
                }
                // When we don't have an interval to update with, set the revalidation options to None.
                None => None,
            };
            value
                .borrow_mut()
                .set_revalidate_options(new_revalidate_options);
        }
    }

    pub(crate) async fn invalidate_queries_inner(
        queries_registry: Signal<QueriesRegistry<T, E, K>>,
        scheduler: Signal<Arc<dyn Fn(ScopeId)>>,
        keys_to_invalidate: Vec<K>,
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

            // Save for the later those listeners that contain all the query keys to invalidate
            if keys_to_invalidate.iter().all(|k| query_keys.contains(k)) {
                for listener in listeners {
                    query_listeners.insert(*listener);
                }
            }

            if !query_listeners.is_empty() {
                value.borrow_mut().set_to_loading();
                for listener in &query_listeners {
                    (scheduler.peek())(*listener);
                }

                // Run the query function
                let fut = (query_fn)(query_keys.clone());
                let fut = Box::into_pin(fut);

                to_owned![query_listeners, value];

                tasks.push(Box::pin(async move {
                    let new_value = fut.await;
                    value.borrow_mut().set_value(new_value.into());

                    for listener in query_listeners {
                        (scheduler.peek())(listener);
                    }
                }));
            }
        }

        tasks.count().await;
    }

    /// Invalidate one or multiple queries that contain all the specified keys.
    /// They will all run concurrently.
    pub fn invalidate_queries(&self, keys_to_invalidate: &[K]) {
        let queries_registry = self.queries_registry;
        let scheduler = self.scheduler;
        let keys_to_invalidate = keys_to_invalidate.to_vec();
        spawn(async move {
            Self::invalidate_queries_inner(
                queries_registry,
                scheduler,
                keys_to_invalidate.to_vec(),
            )
            .await;
        });
    }
}
