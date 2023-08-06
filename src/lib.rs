use dioxus_core::*;
use dioxus_hooks::*;
use futures_util::{future::BoxFuture, stream::StreamExt};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
    ops::Deref,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

const STALE_TIME: u64 = 100;

pub fn use_client<T: Clone + 'static, E: Clone + 'static, K: Clone + 'static>(
    cx: &ScopeState,
) -> &Arc<UseQueryClient<T, E, K>> {
    use_context(cx).unwrap()
}

pub fn use_provide_client<T: Clone + 'static, E: Clone + 'static, K: Clone + 'static>(
    cx: &ScopeState,
) -> &UseQueryClient<T, E, K> {
    let scheduler = cx.use_hook(|| cx.schedule_update_any());

    let coroutine = use_coroutine(cx, {
        move |mut rx: UnboundedReceiver<QueryMutationRequest<T, E, K>>| {
            to_owned![scheduler];
            async move {
                while let Some(mutation) = rx.next().await {
                    // Update to a Loading state
                    match mutation {
                        QueryMutationRequest::Multiple(triggers) => {
                            for QueryData {
                                value,
                                query_fn,
                                query_keys,
                                listeners,
                            } in triggers
                            {
                                // Only change to `Loading` if had been changed at some point
                                if value.borrow().instant.is_some() {
                                    let cached_value: Option<T> = value.borrow().clone().into();
                                    *value.borrow_mut() = CachedResult {
                                        value: QueryResult::Loading(cached_value),
                                        instant: Some(Instant::now()),
                                    };
                                    for s in &listeners {
                                        scheduler(*s);
                                    }
                                }

                                // Fetch the result
                                let new_value = (query_fn)(&query_keys).await;
                                *value.borrow_mut() = CachedResult {
                                    value: new_value,
                                    instant: Some(Instant::now()),
                                };
                                for s in listeners {
                                    scheduler(s);
                                }
                            }
                        }
                        QueryMutationRequest::Creation(QueryData {
                            value,
                            query_fn,
                            query_keys,
                            listeners,
                        }) => {
                            // If it's not fresh, the query runs, otherwise it uses the cached value
                            if !value.borrow().is_fresh() {
                                // Only change to `Loading` if had been changed at some point
                                if value.borrow().instant.is_some() {
                                    let cached_value: Option<T> = value.borrow().clone().into();
                                    *value.borrow_mut() = CachedResult {
                                        value: QueryResult::Loading(cached_value),
                                        instant: Some(Instant::now()),
                                    };
                                    for s in &listeners {
                                        scheduler(*s);
                                    }
                                }

                                // Fetch the result
                                let new_value = (query_fn)(&query_keys).await;
                                *value.borrow_mut() = CachedResult {
                                    value: new_value,
                                    instant: Some(Instant::now()),
                                };
                            }

                            for s in listeners {
                                scheduler(s);
                            }
                        }
                    }
                }
            }
        }
    });

    use_context_provider(cx, || {
        Arc::new(UseQueryClient {
            queries_keys: Rc::default(),
            queries_manager: coroutine.clone(),
        })
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedResult<T, E> {
    value: QueryResult<T, E>,
    instant: Option<Instant>,
}

impl<T, E> CachedResult<T, E> {
    pub fn is_fresh(&self) -> bool {
        if let Some(instant) = self.instant {
            instant.elapsed().as_millis() < Duration::from_millis(STALE_TIME).as_millis()
        } else {
            false
        }
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
        }
    }
}

pub type QueryFn<T, E, K> = dyn Fn(&[K]) -> BoxFuture<QueryResult<T, E>>;

#[derive(Clone)]
struct QueryData<T, E, K> {
    value: Rc<RefCell<CachedResult<T, E>>>,
    listeners: HashSet<ScopeId>,
    query_fn: Arc<QueryFn<T, E, K>>,
    query_keys: Vec<K>,
}

#[derive(Clone)]
struct QueryListeners<T, E, K> {
    value: Rc<RefCell<CachedResult<T, E>>>,
    listeners: HashSet<ScopeId>,
    query_fn: Arc<QueryFn<T, E, K>>,
}

#[derive(Clone)]
enum QueryMutationRequest<T, E, K> {
    /// Invalidate a group of queries
    Multiple(Vec<QueryData<T, E, K>>),
    /// Try to run a query for the first time
    Creation(QueryData<T, E, K>),
}

type QueriesKeys<T, E, K> = HashMap<Vec<K>, QueryListeners<T, E, K>>;

#[derive(Clone)]
pub struct UseQueryClient<T, E, K> {
    queries_keys: Rc<RefCell<QueriesKeys<T, E, K>>>,
    queries_manager: Coroutine<QueryMutationRequest<T, E, K>>,
}

impl<T, E, K: PartialEq + Clone + Eq + Hash> UseQueryClient<T, E, K> {
    fn invalidate_queries_with_mode(&self, keys_to_invalidate: &[K]) {
        let mut actually_subscribed = Vec::default();
        for (
            query_keys,
            QueryListeners {
                value,
                listeners,
                query_fn,
            },
        ) in self.queries_keys.borrow().iter()
        {
            let mut actual_listeners = HashSet::default();

            // Add the listeners of this `query_keys` when at least one of the keys match
            if query_keys.iter().any(|k| keys_to_invalidate.contains(k)) {
                for id in listeners {
                    actual_listeners.insert(*id);
                }
            }

            // Save the group of listeners
            if !actual_listeners.is_empty() {
                actually_subscribed.push(QueryData {
                    value: value.clone(),
                    listeners: actual_listeners,
                    query_fn: query_fn.clone(),
                    query_keys: query_keys.clone(),
                })
            }
        }

        self.queries_manager
            .send(QueryMutationRequest::Multiple(actually_subscribed));
    }

    pub fn invalidate_query(&self, key_to_invalidate: K) {
        self.invalidate_queries_with_mode(&[key_to_invalidate]);
    }

    pub fn invalidate_queries(&self, keys_to_invalidate: &[K]) {
        self.invalidate_queries_with_mode(keys_to_invalidate);
    }
}

pub struct UseValue<T, E, K: Eq + Hash> {
    client: Arc<UseQueryClient<T, E, K>>,
    slot: Rc<RefCell<CachedResult<T, E>>>,
    keys: Vec<K>,
    scope_id: ScopeId,
}

impl<T, E, K: Eq + Hash> Drop for UseValue<T, E, K> {
    fn drop(&mut self) {
        self.client
            .queries_keys
            .borrow_mut()
            .get_mut(&self.keys)
            .unwrap()
            .listeners
            .remove(&self.scope_id);
    }
}

impl<T: Clone, E: Clone, K: Clone + Eq + Hash> UseValue<T, E, K> {
    /// Get the current result from the query.
    pub fn result(&self) -> Ref<CachedResult<T, E>> {
        self.slot.borrow()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum QueryResult<T, E> {
    /// Contains a successful state
    Ok(T),
    /// Contains an errored state
    Err(E),
    /// Contains a loading state that may or not have a cached result
    Loading(Option<T>),
}

impl<T, E> Default for QueryResult<T, E> {
    fn default() -> Self {
        Self::Loading(None)
    }
}

impl<T, E> From<CachedResult<T, E>> for Option<T> {
    fn from(result: CachedResult<T, E>) -> Self {
        match result.value {
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

pub struct QueryConfig<T, E, K> {
    query_keys: Vec<K>,
    query_fn: Arc<Box<QueryFn<T, E, K>>>,
    initial_fn: Option<Box<dyn Fn() -> QueryResult<T, E>>>,
}

impl<T, E, K> QueryConfig<T, E, K> {
    pub fn new(
        keys: Vec<K>,
        query_fn: impl Fn(&[K]) -> BoxFuture<QueryResult<T, E>> + 'static,
    ) -> Self {
        Self {
            query_keys: keys,
            query_fn: Arc::new(Box::new(query_fn)),
            initial_fn: None,
        }
    }

    pub fn initial(mut self, initial_data: impl Fn() -> QueryResult<T, E> + 'static) -> Self {
        self.initial_fn = Some(Box::new(initial_data));
        self
    }
}

/// Get a result given the query config, will re run when the query keys are invalidated.
pub fn use_query_config<T, E, K>(
    cx: &ScopeState,
    config: impl FnOnce() -> QueryConfig<T, E, K>,
) -> &UseValue<T, E, K>
where
    T: 'static + PartialEq + Clone,
    E: 'static + PartialEq + Clone,
    K: Clone + Eq + Hash + 'static,
{
    let client = use_client(cx);
    let config = cx.use_hook(|| Arc::new(config()));

    cx.use_hook(|| {
        let mut binding = client.queries_keys.borrow_mut();
        let query_listeners = binding
            .entry(config.query_keys.clone())
            .or_insert(QueryListeners {
                listeners: HashSet::default(),
                value: Rc::default(),
                query_fn: config.query_fn.clone(),
            });
        query_listeners.listeners.insert(cx.scope_id());

        // Initial async load
        client
            .queries_manager
            .send(QueryMutationRequest::Creation(QueryData {
                value: query_listeners.value.clone(),
                listeners: HashSet::from([cx.scope_id()]),
                query_fn: query_listeners.query_fn.clone(),
                query_keys: config.query_keys.clone(),
            }));

        UseValue {
            client: client.clone(),
            slot: query_listeners.value.clone(),
            keys: config.query_keys.clone(),
            scope_id: cx.scope_id(),
        }
    })
}

/// Get the result of the given query function, will re run when the query keys are invalidated.
pub fn use_query<T: Clone, E: Clone, K>(
    cx: &ScopeState,
    query_keys: impl FnOnce() -> Vec<K>,
    query_fn: impl Fn(&[K]) -> BoxFuture<QueryResult<T, E>> + 'static,
) -> &UseValue<T, E, K>
where
    T: 'static + PartialEq,
    E: 'static + PartialEq,
    K: Clone + Eq + Hash + 'static,
{
    use_query_config(cx, || QueryConfig::new(query_keys(), query_fn))
}
