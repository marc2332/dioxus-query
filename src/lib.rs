use dioxus_core::*;
use dioxus_hooks::*;
use futures_util::stream::StreamExt;
use slab::Slab;
use std::{fmt::Debug, future::Future, rc::Rc, sync::Arc};

pub fn use_client<K: Clone + 'static>(cx: &ScopeState) -> &UseQueryClient<K> {
    use_context(cx).unwrap()
}

pub fn use_provide_client<K: Clone + 'static>(cx: &ScopeState) -> &UseQueryClient<K> {
    use_context_provider(cx, || UseQueryClient {
        queries_keys: Rc::default(),
    })
}

pub type QueryFn<R, K> = dyn Fn(&[K]) -> R + 'static;

#[derive(Clone, Copy)]
enum QueryMutationMode {
    /// An effective mutation will make listening components re run
    Effective,
    /// A silent mutation will not make listening components re run
    Silent,
}

#[derive(Clone)]
pub struct UseQueryClient<K> {
    queries_keys: Rc<RefCell<Slab<Subscriber<K>>>>,
}

impl<K: PartialEq> UseQueryClient<K> {
    fn invalidate_query_with_mode(&self, keys_to_invalidate: &[K], mode: QueryMutationMode) {
        for (_, sub) in self.queries_keys.borrow().iter() {
            if sub.keys.iter().any(|k| keys_to_invalidate.contains(k)) {
                sub.coroutine.send(mode);
            }
        }
    }

    pub fn invalidate_query(&self, key_to_invalidate: K) {
        self.invalidate_query_with_mode(&[key_to_invalidate], QueryMutationMode::Effective);
    }

    pub fn invalidate_queries(&self, keys_to_invalidate: &[K]) {
        self.invalidate_query_with_mode(keys_to_invalidate, QueryMutationMode::Effective);
    }

    pub fn silent_invalidate_query(&self, key_to_invalidate: K) {
        self.invalidate_query_with_mode(&[key_to_invalidate], QueryMutationMode::Silent);
    }

    pub fn silent_invalidate_queries(&self, keys_to_invalidate: &[K]) {
        self.invalidate_query_with_mode(keys_to_invalidate, QueryMutationMode::Silent);
    }
}

pub struct UseValue<T, E, K> {
    client: UseQueryClient<K>,
    slot: UseRef<QueryResult<T, E>>,
    subscriber_id: usize,
}

impl<T, E, K> Drop for UseValue<T, E, K> {
    fn drop(&mut self) {
        self.client
            .queries_keys
            .borrow_mut()
            .remove(self.subscriber_id);
    }
}

impl<T: Clone, E: Clone, K: Clone> UseValue<T, E, K> {
    /// Get the current result from the query.
    pub fn result(&self) -> core::cell::Ref<QueryResult<T, E>> {
        self.slot.read()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum QueryResult<T, E> {
    /// Contains a successful state
    Ok(T),
    /// Contains an errored state
    Err(E),
    /// Contains an empty state
    None,
    /// Contains a loading state that may or not have a cached result
    Loading(Option<T>),
}

impl<T, E> From<QueryResult<T, E>> for Option<T> {
    fn from(value: QueryResult<T, E>) -> Self {
        match value {
            QueryResult::Ok(v) => Some(v),
            QueryResult::Err(_) => None,
            QueryResult::None => None,
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

struct Subscriber<K> {
    keys: Vec<K>,
    coroutine: Coroutine<QueryMutationMode>,
}

pub struct QueryConfig<R, T, E, K> {
    keys: Vec<K>,
    query_fn: Box<QueryFn<R, K>>,
    initial_fn: Option<Box<dyn Fn() -> QueryResult<T, E>>>,
}

impl<R, T, E, K> QueryConfig<R, T, E, K> {
    pub fn new(keys: Vec<K>, query_fn: impl Fn(&[K]) -> R + 'static) -> Self {
        Self {
            keys,
            query_fn: Box::new(query_fn),
            initial_fn: None,
        }
    }

    pub fn initial(mut self, initial_data: impl Fn() -> QueryResult<T, E> + 'static) -> Self {
        self.initial_fn = Some(Box::new(initial_data));
        self
    }
}

/// Get a result given the query config, will re run when the query keys are invalidated.
pub fn use_query_config<R, T, E, K>(
    cx: &ScopeState,
    config: impl FnOnce() -> QueryConfig<R, T, E, K>,
) -> &UseValue<T, E, K>
where
    T: 'static + PartialEq + Clone,
    E: 'static + PartialEq + Clone,
    R: Future<Output = QueryResult<T, E>> + 'static,
    K: Clone + 'static,
{
    let client = use_client(cx);
    let config = cx.use_hook(|| Arc::new(config()));
    let value = use_ref(cx, || {
        // Set an empty state if no initial function is passed
        config
            .initial_fn
            .as_ref()
            .map(|v| v())
            .unwrap_or(QueryResult::None)
    });

    let coroutine = use_coroutine(cx, {
        to_owned![value, config];
        move |mut rx: UnboundedReceiver<QueryMutationMode>| {
            to_owned![value];
            async move {
                while let Some(mutation) = rx.next().await {
                    // Update to a Loading state
                    let cached_value: Option<T> = value.read().clone().into();
                    match mutation {
                        QueryMutationMode::Effective => {
                            *value.write() = QueryResult::Loading(cached_value);
                        }
                        QueryMutationMode::Silent => {
                            *value.write_silent() = QueryResult::Loading(cached_value);
                        }
                    }

                    // Fetch the result
                    let res = (config.query_fn)(&config.keys).await;

                    // Save the result
                    match mutation {
                        QueryMutationMode::Effective => {
                            *value.write() = res;
                        }
                        QueryMutationMode::Silent => {
                            *value.write_silent() = res;
                        }
                    }
                }
            }
        }
    });

    cx.use_hook(|| {
        let subscriber_id = client.queries_keys.borrow_mut().insert(Subscriber {
            keys: config.keys.clone(),
            coroutine: coroutine.clone(),
        });

        // Initial async load
        coroutine.send(QueryMutationMode::Effective);

        UseValue {
            client: client.clone(),
            slot: value.clone(),
            subscriber_id,
        }
    })
}

/// Get the result of the given query function, will re run when the query keys are invalidated.
pub fn use_query<R, T: Clone, E: Clone, K>(
    cx: &ScopeState,
    query_keys: impl FnOnce() -> Vec<K>,
    query_fn: impl Fn(&[K]) -> R + 'static,
) -> &UseValue<T, E, K>
where
    T: 'static + PartialEq,
    E: 'static + PartialEq,
    R: Future<Output = QueryResult<T, E>> + 'static,
    K: Clone + 'static,
{
    use_query_config(cx, || QueryConfig::new(query_keys(), query_fn))
}
