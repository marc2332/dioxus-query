use core::fmt;
use std::{
    cell::{Ref, RefCell},
    collections::{HashMap, HashSet},
    future::Future,
    hash::Hash,
    mem,
    rc::Rc,
    time::{Duration, Instant},
};

use dioxus_lib::hooks::to_owned;
use dioxus_lib::prelude::Task;
use dioxus_lib::prelude::*;
use dioxus_lib::signals::{Readable, Writable};
use dioxus_lib::{
    hooks::{use_memo, use_reactive},
    signals::CopyValue,
};

pub trait QueryCapability
where
    Self: 'static + Clone + PartialEq + Hash + Eq,
{
    type Ok;
    type Err;
    type Keys: Hash + PartialEq + Clone;

    fn run(&self, keys: &Self::Keys) -> impl Future<Output = Result<Self::Ok, Self::Err>>;

    fn matches(&self, _keys: &Self::Keys) -> bool {
        true
    }
}

// TODO: CONSIDER QueryState to be put inside and this being a struct
pub enum QueryStateData<Q: QueryCapability> {
    /// Has not loaded yet.
    Pending,
    /// Is loading and may not have a previous fulfilled value.
    Loading { res: Option<Result<Q::Ok, Q::Err>> },
    /// Is not loading and has a fulfilled value.
    Fulfilled {
        res: Result<Q::Ok, Q::Err>,
        fullfilement_stamp: Instant,
    },
}

// pub enum QueryState<'a, Q: QueryCapability> {
//     /// Has not loaded yet.
//     Pending,
//     /// Is loading and may not have a previous fulfilled value.
//     Loading(&'a Option<Result<Q::Ok, Q::Err>>),
//     /// Is not loading and has a fulfilled value.
//     Fulfilled(&'a Result<Q::Ok, Q::Err>),
// }

impl<Q> fmt::Debug for QueryStateData<Q>
where
    Q: QueryCapability,
    Q::Ok: fmt::Debug,
    Q::Err: fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => f.write_str("Pending"),
            Self::Loading { res } => write!(f, "{res:?}"),
            Self::Fulfilled { res, .. } => write!(f, "{res:?}"),
        }
    }
}

impl<Q: QueryCapability> QueryStateData<Q> {
    pub(crate) fn into_loading(self) -> QueryStateData<Q> {
        match self {
            QueryStateData::Pending => QueryStateData::Loading { res: None },
            QueryStateData::Loading { res } => QueryStateData::Loading { res },
            QueryStateData::Fulfilled { res, .. } => QueryStateData::Loading { res: Some(res) },
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, QueryStateData::Fulfilled { res: Ok(_), .. })
    }

    pub fn is_err(&self) -> bool {
        matches!(self, QueryStateData::Fulfilled { res: Err(_), .. })
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, QueryStateData::Loading { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, QueryStateData::Pending)
    }

    pub fn is_stale(&self, query: &Query<Q>) -> bool {
        match self {
            QueryStateData::Pending => true,
            QueryStateData::Loading { .. } => false,
            QueryStateData::Fulfilled {
                fullfilement_stamp, ..
            } => Instant::now().duration_since(*fullfilement_stamp) >= query.stale_time,
        }
    }

    pub fn ok(&self) -> Option<&Q::Ok> {
        match self {
            Self::Fulfilled { res: Ok(res), .. } => Some(res),
            Self::Loading { res: Some(Ok(res)) } => Some(res),
            _ => None,
        }
    }

    pub fn unwrap(&self) -> &Result<Q::Ok, Q::Err> {
        match self {
            Self::Loading { res: Some(v) } => v,
            Self::Fulfilled { res, .. } => res,
            _ => unreachable!(),
        }
    }
}
pub struct QueriesStorage<Q: QueryCapability> {
    storage: CopyValue<HashMap<Query<Q>, QueryData<Q>>>,
}

impl<Q: QueryCapability> Copy for QueriesStorage<Q> {}

impl<Q: QueryCapability> Clone for QueriesStorage<Q> {
    fn clone(&self) -> Self {
        *self
    }
}

pub struct QueryData<Q: QueryCapability> {
    state: Rc<RefCell<QueryStateData<Q>>>,
    scopes: HashSet<ScopeId>,

    clean_task: Option<Task>,
}

impl<Q: QueryCapability> Clone for QueryData<Q> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            scopes: self.scopes.clone(),
            clean_task: self.clean_task,
        }
    }
}

impl<Q: QueryCapability> QueriesStorage<Q> {
    fn new_in_root() -> Self {
        Self {
            storage: CopyValue::new_in_scope(HashMap::default(), ScopeId::ROOT),
        }
    }

    fn insert_subscription(&mut self, query: Query<Q>, scope_id: ScopeId) -> QueryData<Q> {
        let mut storage = self.storage.write();
        let data = storage.entry(query).or_insert_with(|| QueryData {
            state: Rc::new(RefCell::new(QueryStateData::Pending)),
            scopes: HashSet::new(),
            clean_task: None,
        });

        // Subscribe scope
        data.scopes.insert(scope_id);

        // Cancel clean task
        if let Some(clean_task) = data.clean_task {
            clean_task.cancel();
        }

        data.clone()
    }

    fn remove_subscription(&mut self, query: Query<Q>, scope_id: ScopeId) {
        let mut storage_clone = self.storage;
        let mut storage = self.storage.write();

        // Remove scope
        let query_data = storage.get_mut(&query).unwrap();
        query_data.scopes.remove(&scope_id);

        // Spawn clean up task if there no more scopes
        if query_data.scopes.is_empty() {
            query_data.clean_task = Some(spawn(async move {
                // Wait as long as the stale time is configured
                #[cfg(not(target_family = "wasm"))]
                tokio::time::sleep(query.stale_time).await;
                #[cfg(target_family = "wasm")]
                wasmtimer::tokio::sleep(query.stale_time).await;

                // Finally clear the query
                let mut storage = storage_clone.write();
                storage.remove(&query);
            }));
        }
    }

    async fn run(&mut self, query: &Query<Q>, data: QueryData<Q>) {
        let res = query.query.run(&query.keys).await;
        *data.state.borrow_mut() = QueryStateData::Fulfilled {
            res,
            fullfilement_stamp: Instant::now(),
        };

        // TODO: Possible issue, what if the data scopes is too outdated at this point
        let cb = schedule_update_any();
        for scope_id in data.scopes {
            cb(scope_id)
        }
    }

    pub async fn invalidate_all() {
        let storage = consume_context::<QueriesStorage<Q>>();

        // Get all the queries
        let matching_queries = storage
            .storage
            .read()
            .clone()
            .into_iter()
            .collect::<Vec<_>>();

        // Invalidate the queries
        Self::run_queries(&matching_queries).await
    }

    pub async fn invalidate_matching(matching_keys: Q::Keys) {
        let storage = consume_context::<QueriesStorage<Q>>();

        // Get those queries that match
        let mut matching_queries = Vec::new();
        for (query, data) in storage.storage.read().iter() {
            if query.query.matches(&matching_keys) {
                matching_queries.push((query.clone(), data.clone()));
            }
        }

        // Invalidate the queries
        Self::run_queries(&matching_queries).await
    }

    async fn run_queries(queries: &[(Query<Q>, QueryData<Q>)]) {
        let cb = schedule_update_any();
        for (query, data) in queries {
            // Set to Loading
            let res =
                mem::replace(&mut *data.state.borrow_mut(), QueryStateData::Pending).into_loading();
            *data.state.borrow_mut() = res;
            for scope_id in &data.scopes {
                cb(*scope_id)
            }

            // Run and set to fulfilled
            let res = query.query.run(&query.keys).await;
            *data.state.borrow_mut() = QueryStateData::Fulfilled {
                res,
                fullfilement_stamp: Instant::now(),
            };
            for scope_id in &data.scopes {
                cb(*scope_id)
            }
        }
    }
}

#[derive(PartialEq, Clone)]
pub struct Query<Q: QueryCapability> {
    query: Q,
    keys: Q::Keys,

    stale_time: Duration,
}

impl<Q: QueryCapability> Eq for Query<Q> {}
impl<Q: QueryCapability> Hash for Query<Q> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.query.hash(state);
        self.query.hash(state);
    }
}

impl<Q: QueryCapability> Query<Q> {
    pub fn new(keys: Q::Keys, query: Q) -> Self {
        Self {
            query,
            keys,
            stale_time: Duration::ZERO,
        }
    }

    pub fn stale_time(self, stale_time: Duration) -> Self {
        Self { stale_time, ..self }
    }
}

pub struct QueryReader<Q: QueryCapability> {
    state: Rc<RefCell<QueryStateData<Q>>>,
}

impl<Q: QueryCapability> QueryReader<Q> {
    pub fn state(&self) -> Ref<QueryStateData<Q>> {
        self.state.borrow()
    }
}

pub struct UseQuery<Q: QueryCapability> {
    query: Query<Q>,
}

impl<Q: QueryCapability> UseQuery<Q> {
    pub fn read(&self) -> QueryReader<Q> {
        let storage = consume_context::<QueriesStorage<Q>>();
        let state = storage
            .storage
            .peek_unchecked()
            .get(&self.query)
            .cloned()
            .unwrap();
        QueryReader { state: state.state }
    }
}

pub fn use_query<Q: QueryCapability>(query: Query<Q>) -> UseQuery<Q> {
    let mut storage = match try_consume_context::<QueriesStorage<Q>>() {
        Some(storage) => storage,
        None => provide_root_context(QueriesStorage::<Q>::new_in_root()),
    };

    let scope_id = current_scope_id().unwrap();

    // Create or update query subscrition on changes
    use_memo(use_reactive!(|query| {
        let data = storage.insert_subscription(query.clone(), scope_id);

        // Immediately run the query if the value is stale
        if data.state.borrow().is_stale(&query) {
            spawn(async move {
                storage.run(&query, data).await;
            });
        }
    }));

    // Remove query subscription on scope drop
    use_drop({
        to_owned![query];
        move || {
            storage.remove_subscription(query, scope_id);
        }
    });

    UseQuery { query }
}
