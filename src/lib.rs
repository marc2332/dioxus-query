use dioxus_core::*;
use dioxus_hooks::*;
pub use futures_util;
use futures_util::{
    future::BoxFuture,
    stream::{FuturesUnordered, StreamExt},
};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
    ops::Deref,
    rc::Rc,
    sync::{Arc, RwLock, RwLockReadGuard},
    time::{Duration, Instant},
};

const STALE_TIME: u64 = 100;

/// Get access to the **UseQueryClient**.
pub fn use_query_client<T: 'static, E: 'static, K: 'static>(
    cx: &ScopeState,
) -> &Arc<UseQueryClient<T, E, K>> {
    use_context(cx).unwrap()
}

/// Provide a **UseQueryClient** to your app.
pub fn use_provide_query_client<T: 'static, E: 'static, K: 'static>(
    cx: &ScopeState,
) -> &UseQueryClient<T, E, K> {
    use_context_provider(cx, || {
        Arc::new(UseQueryClient {
            queries_registry: Rc::default(),
            scheduler: cx.schedule_update_any(),
        })
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedResult<T, E> {
    value: QueryResult<T, E>,
    instant: Option<Instant>,
}

impl<T, E> CachedResult<T, E> {
    pub fn value(&self) -> &QueryResult<T, E> {
        &self.value
    }

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

pub type QueryFn<T, E, K> = dyn Fn(&[K]) -> BoxFuture<QueryResult<T, E>> + Send + Sync;

type QueryValue<T> = Arc<RwLock<T>>;

#[derive(Clone)]
struct QueryListeners<T, E, K> {
    value: QueryValue<CachedResult<T, E>>,
    listeners: HashSet<ScopeId>,
    query_fn: Arc<Box<QueryFn<T, E, K>>>,
}

type QueriesRegistry<T, E, K> = HashMap<Vec<K>, QueryListeners<T, E, K>>;

#[derive(Clone)]
pub struct UseQueryClient<T, E, K> {
    queries_registry: Rc<RefCell<QueriesRegistry<T, E, K>>>,
    scheduler: Arc<dyn Fn(ScopeId) + Send + Sync>,
}

impl<T: Clone + 'static, E: Clone + 'static, K: PartialEq + Clone + 'static>
    UseQueryClient<T, E, K>
{
    async fn validate_new_query(
        &self,
        value: QueryValue<CachedResult<T, E>>,
        query_listener: ScopeId,
        query_fn: Arc<Box<QueryFn<T, E, K>>>,
        query_keys: Vec<K>,
    ) {
        // If it's not fresh, the query runs, otherwise it uses the cached value
        if !value.read().unwrap().is_fresh() {
            // Only change to `Loading` if had been changed at some point
            if value.read().unwrap().instant.is_some() {
                let cached_value: Option<T> = value.read().unwrap().clone().into();
                *value.write().unwrap() = CachedResult {
                    value: QueryResult::Loading(cached_value),
                    instant: Some(Instant::now()),
                };
                (self.scheduler)(query_listener);
            }

            let scheduler = self.scheduler.clone();

            // Fetch the result
            let new_value = (query_fn)(&query_keys).await;
            *value.write().unwrap() = CachedResult {
                value: new_value,
                instant: Some(Instant::now()),
            };

            (scheduler)(query_listener);
        } else {
            (self.scheduler)(query_listener);
        }
    }

    async fn invalidate_queries_inner(&self, keys_to_invalidate: &[K]) {
        let tasks = FuturesUnordered::new();
        for (
            query_keys,
            QueryListeners {
                value,
                listeners,
                query_fn,
            },
        ) in self.queries_registry.borrow().iter()
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
                };
                for listener in &query_listeners {
                    (self.scheduler)(*listener);
                }

                let scheduler = self.scheduler.clone();
                to_owned![query_fn, query_keys, query_listeners, value];

                tasks.push(Box::pin(async move {
                    // Fetch the result
                    let new_value = (query_fn)(&query_keys).await;
                    *value.write().unwrap() = CachedResult {
                        value: new_value,
                        instant: Some(Instant::now()),
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
    pub async fn invalidate_query(&self, key_to_invalidate: K) {
        self.invalidate_queries_inner(&[key_to_invalidate]).await;
    }

    /// Invalidate a group of queries.
    /// They will all run concurrently, after previous queries have finished.
    pub async fn invalidate_queries(&self, keys_to_invalidate: &[K]) {
        self.invalidate_queries_inner(keys_to_invalidate).await;
    }
}

pub struct UseValue<T, E, K: Eq + Hash> {
    client: Arc<UseQueryClient<T, E, K>>,
    value: QueryValue<CachedResult<T, E>>,
    keys: Vec<K>,
    scope_id: ScopeId,
}

impl<T, E, K: Eq + Hash> Drop for UseValue<T, E, K> {
    fn drop(&mut self) {
        let is_empty = {
            let mut queries_registry = self.client.queries_registry.borrow_mut();
            let query_listeners = queries_registry.get_mut(&self.keys).unwrap();
            // Remove this `UseValue`'s listener
            query_listeners.listeners.remove(&self.scope_id);
            query_listeners.listeners.is_empty()
        };
        if is_empty {
            // Remove the query keys if this was the last listener listening
            self.client.queries_registry.borrow_mut().remove(&self.keys);
        }
    }
}

impl<T, E, K: Eq + Hash> UseValue<T, E, K> {
    /// Get the current result from the query.
    pub fn result(&self) -> RwLockReadGuard<CachedResult<T, E>> {
        self.value.read().unwrap()
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
        query_fn: impl Fn(&[K]) -> BoxFuture<QueryResult<T, E>> + 'static + Send + Sync,
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
    let client = use_query_client(cx);
    let config = cx.use_hook(|| Arc::new(config()));

    cx.use_hook(|| {
        let mut queries_registry = client.queries_registry.borrow_mut();
        // Create a group of listeners for the given combination of keys
        let query_listeners =
            queries_registry
                .entry(config.query_keys.clone())
                .or_insert(QueryListeners {
                    listeners: HashSet::default(),
                    value: QueryValue::default(),
                    query_fn: config.query_fn.clone(),
                });
        // Register this component as listener of the keys combination
        query_listeners.listeners.insert(cx.scope_id());

        // Initial async load
        cx.spawn({
            let client = client.clone();
            let query_listeners = query_listeners.clone();
            let scope_id = cx.scope_id();
            let query_keys = config.query_keys.clone();
            async move {
                client
                    .validate_new_query(
                        query_listeners.value.clone(),
                        scope_id,
                        query_listeners.query_fn.clone(),
                        query_keys,
                    )
                    .await;
            }
        });

        UseValue {
            client: client.clone(),
            value: query_listeners.value.clone(),
            keys: config.query_keys.clone(),
            scope_id: cx.scope_id(),
        }
    })
}

/// Register a query listener with the given combination of **query keys** and **query function**.
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
) -> &UseValue<T, E, K>
where
    T: 'static + PartialEq,
    E: 'static + PartialEq,
    K: Clone + Eq + Hash + 'static,
{
    use_query_config(cx, || QueryConfig::new(query_keys(), query_fn))
}

pub type MutationFn<T, E, P> = dyn Fn(P) -> BoxFuture<'static, MutationResult<T, E>>;

/// Manage a mutation.
#[derive(Clone)]
pub struct UseMutation<T, E, P> {
    value: Rc<RefCell<MutationResult<T, E>>>,
    mutation_fn: Arc<Box<MutationFn<T, E, P>>>,
    scheduler: Arc<dyn Fn(ScopeId) + Send + Sync>,
    scope_id: ScopeId,
}

impl<T: Clone, E: Clone, P> UseMutation<T, E, P> {

    /// Get the current result from the query.
    pub fn result(&self) -> Ref<'_, MutationResult<T, E>> {
        self.value.borrow()
    }

    /// Call the mutation function with a set of arguments.
    pub async fn mutate(&self, arg: P) -> Ref<'_, MutationResult<T, E>> {
        let cached_value = self.value.borrow().clone().into();

        // Set state to loading and notify
        *self.value.borrow_mut() = MutationResult::Loading(cached_value);
        // TODO optimization: Check if the value was already loading
        // to decide to call the scheduler or not
        (self.scheduler)(self.scope_id);

        // Trigger the mutation function
        let value = (self.mutation_fn)(arg).await;

        // Set state to the new value and notify
        *self.value.borrow_mut() = value;
        // TODO optimization: Check if the previous and new value are
        // different to decide to call the scheduler or not
        (self.scheduler)(self.scope_id);

        self.value.borrow()
    }

    /// Call the mutation function silently with a set of arguments.
    /// This will not make the component re run.
    pub async fn mutate_silent(&self, arg: P) -> Ref<'_, MutationResult<T, E>> {
        let cached_value = self.value.borrow().clone().into();

        // Set state to loading
        *self.value.borrow_mut() = MutationResult::Loading(cached_value);

        // Trigger the mutation function
        let value = (self.mutation_fn)(arg).await;

        // Set state to the new value
        *self.value.borrow_mut() = value;

        self.value.borrow()
    }
}

/// The result of mutation.
#[derive(Clone, PartialEq, Debug)]
pub enum MutationResult<T, E> {
    /// Mutation was successful
    Ok(T),
    /// Mutation erorred
    Err(E),
    /// Mutation is loading and may or not have a previous result
    Loading(Option<T>),
    /// Mutation has not been triggered yet
    Pending,
}

impl<T, E> MutationResult<T, E> {
    pub fn is_ok(&self) -> bool {
        matches!(self, MutationResult::Ok(..))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, MutationResult::Err(..))
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, MutationResult::Loading(..))
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, MutationResult::Pending)
    }
}

impl<T, E> From<Result<T, E>> for MutationResult<T, E> {
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(v) => MutationResult::Ok(v),
            Err(e) => MutationResult::Err(e),
        }
    }
}

impl<T, E> From<MutationResult<T, E>> for Option<T> {
    fn from(result: MutationResult<T, E>) -> Self {
        match result {
            MutationResult::Ok(v) => Some(v),
            MutationResult::Err(_) => None,
            MutationResult::Loading(v) => v,
            MutationResult::Pending => None,
        }
    }
}

/// Manage a mutation
pub fn use_mutation<T, E, P>(
    cx: &ScopeState,
    mutation_fn: impl Fn(P) -> BoxFuture<'static, MutationResult<T, E>> + 'static,
) -> &UseMutation<T, E, P>
where
    T: 'static + PartialEq,
    E: 'static + PartialEq,
    P: 'static,
{
    let value = cx.use_hook(|| Rc::new(RefCell::new(MutationResult::Pending)));

    cx.use_hook(|| UseMutation {
        value: value.clone(),
        mutation_fn: Arc::new(Box::new(mutation_fn)),
        scheduler: cx.schedule_update_any(),
        scope_id: cx.scope_id(),
    })
}
