use dioxus_core::*;
use dioxus_hooks::*;
use futures_util::{
    future::{BoxFuture, self},
    stream::{FuturesUnordered, StreamExt, FuturesOrdered, self},
};
use tokio::time::sleep;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    hash::Hash,
    ops::Deref,
    rc::Rc,
    sync::{Arc, Mutex, MutexGuard, RwLock, LockResult, RwLockReadGuard, RwLockWriteGuard},
    time::{Duration, Instant},
};


const STALE_TIME: u64 = 100;

pub fn use_client<T: Clone + 'static, E: Clone + 'static, K: Clone + 'static>(
    cx: &ScopeState,
) -> &Arc<UseQueryClient<T, E, K>> {
    use_context(cx).unwrap()
}

struct QueuedLock<T: ?Sized> {
    value: Arc<RwLock<T>>,
    queue: Arc<RwLock<Vec<Box<dyn FnOnce(&mut T)>>>>
}

impl<T> Clone for QueuedLock<T> {
    fn clone_from(&mut self, source: &Self) {
        *self = source.clone()
    }

    fn clone(&self) -> Self {
        Self { value: self.value.clone(), queue: self.queue.clone() }
    }
}

/// TODO: Implement deref
struct QueuedLockGuard<'a, T> {
    lock: RwLockWriteGuard<'a, T>,
    queue: Arc<RwLock<Vec<Box<dyn FnOnce(&mut T)>>>>
}

impl<T> Drop for QueuedLockGuard<'_, T> {
    fn drop(&mut self) {
        for cb in self.queue.write().unwrap().drain(..) {
            cb(&mut self.lock)
        }
    }
}

impl<T> QueuedLock<T> {

    pub fn new(v: T) -> Self {
        Self {
            value: Arc::new(RwLock::new(v)),
            queue: Arc::default()
        }
    }

    pub fn with(&self, cb: impl FnOnce(&mut T) + 'static) {
        
        self.queue.write().unwrap().push(Box::new(cb));

        if let Ok(mut v) = self.value.try_write() {
            for cb in self.queue.write().unwrap().drain(..) {
                cb(&mut v)
            }
        }
    }

    pub fn read(&self)  -> QueuedLockGuard<T> {
        QueuedLockGuard {
            lock: self.value.write().unwrap(),
            queue: self.queue.clone()
        }
    }
}


pub fn use_provide_client<T: Clone + 'static, E: Clone + 'static, K: Clone + 'static>(
    cx: &ScopeState,
) -> &UseQueryClient<T, E, K> {
    let scheduler = cx.use_hook(|| cx.schedule_update_any());
    let manager = cx.use_hook(|| QueuedLock::new(FuturesOrdered::<BoxFuture<()>>::new()));

    let coroutine = use_coroutine(cx, {
        let manager = manager.clone();
        move |mut rx: UnboundedReceiver<()>| {
            to_owned![manager];
            async move {
                while rx.next().await.is_some() {
                    while manager.read().lock.next().await.is_some() {
                        println!(".");
                    }
                }
            }
        }
    });

    use_context_provider(cx, || {
        Arc::new(UseQueryClient {
            queries_keys: Rc::default(),
            manager2: manager.clone(),
            scheduler: scheduler.clone(),
            coroutine: coroutine.clone()
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

pub type QueryFn<T, E, K> = dyn Fn(&[K]) -> BoxFuture<QueryResult<T, E>> + Send + Sync;

#[derive(Clone)]
struct QueryData<T, E, K> {
    value: QueryValue<CachedResult<T, E>>,
    listeners: HashSet<ScopeId>,
    query_fn: Arc<Box<QueryFn<T, E, K>>>,
    query_keys: Vec<K>,
}

/// TODO: Implement deref
#[derive(Clone, Default)]
struct QueryValue<T>(Arc<RwLock<T>>);

#[derive(Clone)]
struct QueryListeners<T, E, K> {
    value: QueryValue<CachedResult<T, E>>,
    listeners: HashSet<ScopeId>,
    query_fn: Arc<Box<QueryFn<T, E, K>>>,
}


type QueriesKeys<T, E, K> = HashMap<Vec<K>, QueryListeners<T, E, K>>;

#[derive(Clone)]
pub struct UseQueryClient<T, E, K> {
    queries_keys: Rc<RefCell<QueriesKeys<T, E, K>>>,
    manager2: QueuedLock<FuturesOrdered<BoxFuture<'static, ()>>>,
    scheduler: Arc<dyn Fn(ScopeId) + Send + Sync>,
    coroutine: Coroutine<()>
}

impl<
        T: Clone + Send + Sync + 'static,
        E: Clone + Send + Sync + 'static,
        K: PartialEq + Clone + Eq + Hash + Send + Sync + 'static,
    > UseQueryClient<T, E, K>
{
    fn validate_new_query(&self, QueryData { value, listeners, query_fn, query_keys }: QueryData<T, E, K>) {
        // If it's not fresh, the query runs, otherwise it uses the cached value
        if !value.0.read().unwrap().is_fresh() {
            // Only change to `Loading` if had been changed at some point
            if value.0.read().unwrap().instant.is_some() {
                let cached_value: Option<T> = value.0.read().unwrap().clone().into();
                *value.0.write().unwrap() = CachedResult {
                    value: QueryResult::Loading(cached_value),
                    instant: Some(Instant::now()),
                };
                for s in &listeners {
                    (self.scheduler)(*s);
                }
            }

            let scheduler = self.scheduler.clone();
            let coroutine = self.coroutine.clone();

            self.manager2.with(move |tasks| {
                tasks.push_back(Box::pin(async move {
                    // Fetch the result
                   let new_value = (query_fn)(&query_keys).await;
                   *value.0.write().unwrap() = CachedResult {
                       value: new_value,
                       instant: Some(Instant::now()),
                   };
   
                   for s in listeners {
                       scheduler(s);
                   }
               }));
               coroutine.send(());
            })

            

           
        } else {
            for s in listeners {
                (self.scheduler)(s);
            }
        }

        
    }

    fn invalidate_queries_inner(&self, keys_to_invalidate: &[K]) {
        let mut triggers = Vec::default();
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
                triggers.push(QueryData {
                    value: value.clone(),
                    listeners: actual_listeners,
                    query_fn: query_fn.clone(),
                    query_keys: query_keys.clone(),
                })
            }
        }

        for QueryData {
            value,
            query_fn,
            query_keys,
            listeners,
        } in triggers.drain(..)
        {
            // Only change to `Loading` if had been changed at some point
            if value.0.read().unwrap().instant.is_some() {
                let cached_value: Option<T> = value.0.read().unwrap().clone().into();
                *value.0.write().unwrap() = CachedResult {
                    value: QueryResult::Loading(cached_value),
                    instant: Some(Instant::now()),
                };
                for s in &listeners {
                    (self.scheduler)(*s);
                }
            }

            let scheduler = self.scheduler.clone();
            let value = value.clone();
            let coroutine = self.coroutine.clone();


            self.manager2.with(move |tasks| {
                tasks.push_back(Box::pin(async move {
                    // Fetch the result
                    let new_value = (query_fn)(&query_keys).await;
                    *value.0.write().unwrap() = CachedResult {
                        value: new_value,
                        instant: Some(Instant::now()),
                    };
                    for s in listeners {
                        scheduler(s);
                    }
                }));
                coroutine.send(());
            })

            
        }
    }

    pub fn invalidate_query(&self, key_to_invalidate: K) {
        self.invalidate_queries_inner(&[key_to_invalidate]);
    }

    pub fn invalidate_queries(&self, keys_to_invalidate: &[K]) {
        self.invalidate_queries_inner(keys_to_invalidate);
    }
}

pub struct UseValue<T, E, K: Eq + Hash> {
    client: Arc<UseQueryClient<T, E, K>>,
    slot: QueryValue<CachedResult<T, E>>,
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
    pub fn result(&self) -> RwLockReadGuard<CachedResult<T, E>> {
        self.slot.0.read().unwrap()
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

impl<T: Send + Sync, E: Send + Sync, K: Send + Sync> QueryConfig<T, E, K> {
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
pub fn use_query_config<T: Send + Sync, E: Send + Sync, K: Send + Sync>(
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
                value: QueryValue::default(),
                query_fn: config.query_fn.clone(),
            });
        query_listeners.listeners.insert(cx.scope_id());

        // Initial async load
        client.validate_new_query(QueryData {
            value: query_listeners.value.clone(),
            listeners: HashSet::from([cx.scope_id()]),
            query_fn: query_listeners.query_fn.clone(),
            query_keys: config.query_keys.clone(),
        });

        UseValue {
            client: client.clone(),
            slot: query_listeners.value.clone(),
            keys: config.query_keys.clone(),
            scope_id: cx.scope_id(),
        }
    })
}

/// Get the result of the given query function, will re run when the query keys are invalidated.
pub fn use_query<T: Clone + Send + Sync, E: Clone + Send + Sync, K: Send + Sync>(
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
