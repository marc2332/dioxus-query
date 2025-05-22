use core::fmt;
use std::{
    cell::{Ref, RefCell},
    collections::{HashMap, HashSet},
    future::Future,
    hash::Hash,
    mem,
    rc::Rc,
};

use dioxus_lib::prelude::Task;
use dioxus_lib::prelude::*;
use dioxus_lib::signals::{Readable, Writable};
use dioxus_lib::{
    hooks::{use_memo, use_reactive},
    signals::CopyValue,
};
use web_time::{Duration, Instant};

pub trait MutationCapability
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

    fn on_settled(
        &self,
        _keys: &Self::Keys,
        _result: &Result<Self::Ok, Self::Err>,
    ) -> impl Future<Output = ()> {
        async {}
    }
}

pub enum MutationStateData<Q: MutationCapability> {
    /// Has not loaded yet.
    Pending,
    /// Is loading and may not have a previous settled value.
    Loading { res: Option<Result<Q::Ok, Q::Err>> },
    /// Is not loading and has a settled value.
    Settled {
        res: Result<Q::Ok, Q::Err>,
        settlement_instant: Instant,
    },
}

impl<Q> fmt::Debug for MutationStateData<Q>
where
    Q: MutationCapability,
    Q::Ok: fmt::Debug,
    Q::Err: fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => f.write_str("Pending"),
            Self::Loading { res } => write!(f, "Loading {{ {res:?} }}"),
            Self::Settled { res, .. } => write!(f, "Settled {{ {res:?} }}"),
        }
    }
}

impl<Q: MutationCapability> MutationStateData<Q> {
    /// Check if the state is [MutationStateData::Settled] and [Result::Ok].
    pub fn is_ok(&self) -> bool {
        matches!(self, MutationStateData::Settled { res: Ok(_), .. })
    }

    /// Check if the state is [MutationStateData::Settled] and [Result::Err].
    pub fn is_err(&self) -> bool {
        matches!(self, MutationStateData::Settled { res: Err(_), .. })
    }

    /// Check if the state is [MutationStateData::Loading].
    pub fn is_loading(&self) -> bool {
        matches!(self, MutationStateData::Loading { .. })
    }

    /// Check if the state is [MutationStateData::Pending].
    pub fn is_pending(&self) -> bool {
        matches!(self, MutationStateData::Pending)
    }

    /// Get the value as an [Option].
    pub fn ok(&self) -> Option<&Q::Ok> {
        match self {
            Self::Settled { res: Ok(res), .. } => Some(res),
            Self::Loading { res: Some(Ok(res)) } => Some(res),
            _ => None,
        }
    }

    /// Get the value as an [Result] if possible, otherwise it will panic.
    pub fn unwrap(&self) -> &Result<Q::Ok, Q::Err> {
        match self {
            Self::Loading { res: Some(v) } => v,
            Self::Settled { res, .. } => res,
            _ => unreachable!(),
        }
    }

    fn into_loading(self) -> MutationStateData<Q> {
        match self {
            MutationStateData::Pending => MutationStateData::Loading { res: None },
            MutationStateData::Loading { res } => MutationStateData::Loading { res },
            MutationStateData::Settled { res, .. } => MutationStateData::Loading { res: Some(res) },
        }
    }
}
pub struct MutationsStorage<Q: MutationCapability> {
    storage: CopyValue<HashMap<Mutation<Q>, MutationData<Q>>>,
}

impl<Q: MutationCapability> Copy for MutationsStorage<Q> {}

impl<Q: MutationCapability> Clone for MutationsStorage<Q> {
    fn clone(&self) -> Self {
        *self
    }
}

pub struct MutationData<Q: MutationCapability> {
    state: Rc<RefCell<MutationStateData<Q>>>,
    scopes: Rc<RefCell<HashSet<ScopeId>>>,

    clean_task: Option<Task>,
}

impl<Q: MutationCapability> Clone for MutationData<Q> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            scopes: self.scopes.clone(),
            clean_task: self.clean_task,
        }
    }
}

impl<Q: MutationCapability> MutationsStorage<Q> {
    fn new_in_root() -> Self {
        Self {
            storage: CopyValue::new_in_scope(HashMap::default(), ScopeId::ROOT),
        }
    }

    fn insert_subscription(&mut self, mutation: Mutation<Q>, scope_id: ScopeId) -> MutationData<Q> {
        let mut storage = self.storage.write();

        let mutation_data = storage.entry(mutation).or_insert_with(|| MutationData {
            state: Rc::new(RefCell::new(MutationStateData::Pending)),
            scopes: Rc::default(),
            clean_task: None,
        });

        // Subscribe scope
        mutation_data.scopes.borrow_mut().insert(scope_id);

        // Cancel clean task
        if let Some(clean_task) = mutation_data.clean_task.take() {
            clean_task.cancel();
        }

        mutation_data.clone()
    }

    fn remove_subscription(&mut self, mutation: Mutation<Q>, scope_id: ScopeId) {
        let mut storage_clone = self.storage;
        let mut storage = self.storage.write();

        // Remove scope
        let mutation_data = storage.get_mut(&mutation).unwrap();
        mutation_data.scopes.borrow_mut().remove(&scope_id);

        // Spawn clean up task if there no more scopes
        if mutation_data.scopes.borrow().is_empty() {
            mutation_data.clean_task = spawn_forever(async move {
                // Wait as long as the stale time is configured
                #[cfg(not(target_family = "wasm"))]
                tokio::time::sleep(mutation.clean_time).await;
                #[cfg(target_family = "wasm")]
                wasmtimer::tokio::sleep(mutation.clean_time).await;

                // Finally clear the mutation
                let mut storage = storage_clone.write();
                storage.remove(&mutation);
            });
        }
    }

    async fn run(&mut self, mutation: &Mutation<Q>, data: &MutationData<Q>) {
        let cb = schedule_update_any();

        // Set to Loading
        let res =
            mem::replace(&mut *data.state.borrow_mut(), MutationStateData::Pending).into_loading();
        *data.state.borrow_mut() = res;
        for scope_id in data.scopes.borrow().iter() {
            cb(*scope_id)
        }

        // Run
        let res = mutation.mutation.run(&mutation.keys).await;

        // Set to Settled
        mutation.mutation.on_settled(&mutation.keys, &res).await;
        *data.state.borrow_mut() = MutationStateData::Settled {
            res,
            settlement_instant: Instant::now(),
        };
        for scope_id in data.scopes.borrow().iter() {
            cb(*scope_id)
        }
    }
}

#[derive(PartialEq, Clone)]
pub struct Mutation<Q: MutationCapability> {
    mutation: Q,
    keys: Q::Keys,

    clean_time: Duration,
}

impl<Q: MutationCapability> Eq for Mutation<Q> {}
impl<Q: MutationCapability> Hash for Mutation<Q> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.mutation.hash(state);
        self.mutation.hash(state);
    }
}

impl<Q: MutationCapability> Mutation<Q> {
    pub fn new(keys: Q::Keys, mutation: Q) -> Self {
        Self {
            mutation,
            keys,
            clean_time: Duration::ZERO,
        }
    }

    /// For how long the data is kept cached after there are no more mutation subscribers.
    ///
    /// Defaults to [Duration::ZERO], meaning it clears automatically.
    pub fn clean_time(self, clean_time: Duration) -> Self {
        Self { clean_time, ..self }
    }
}

pub struct MutationReader<Q: MutationCapability> {
    state: Rc<RefCell<MutationStateData<Q>>>,
}

impl<Q: MutationCapability> MutationReader<Q> {
    pub fn state(&self) -> Ref<MutationStateData<Q>> {
        self.state.borrow()
    }
}

pub struct UseMutation<Q: MutationCapability> {
    mutation: Memo<Mutation<Q>>,
}

impl<Q: MutationCapability> Clone for UseMutation<Q> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Q: MutationCapability> Copy for UseMutation<Q> {}

impl<Q: MutationCapability> UseMutation<Q> {
    pub fn read(&self) -> MutationReader<Q> {
        let storage = consume_context::<MutationsStorage<Q>>();
        let state = storage
            .storage
            .peek_unchecked()
            .get(&self.mutation.peek())
            .cloned()
            .unwrap();
        MutationReader { state: state.state }
    }

    pub async fn mutate_async(&self) -> MutationReader<Q> {
        let mut storage = consume_context::<MutationsStorage<Q>>();
        let data = storage
            .storage
            .peek_unchecked()
            .get(&self.mutation.peek())
            .cloned()
            .unwrap();
        storage.run(&self.mutation.peek(), &data).await;
        MutationReader { state: data.state }
    }
}

pub fn use_mutation<Q: MutationCapability>(mutation: Mutation<Q>) -> UseMutation<Q> {
    let mut storage = match try_consume_context::<MutationsStorage<Q>>() {
        Some(storage) => storage,
        None => provide_root_context(MutationsStorage::<Q>::new_in_root()),
    };

    let scope_id = current_scope_id().unwrap();

    let current_mutation = use_hook(|| Rc::new(RefCell::new(None)));

    // Create or update mutation subscription on changes
    let mutation = use_memo(use_reactive!(|mutation| {
        let _data = storage.insert_subscription(mutation.clone(), scope_id);

        // Remove the current mutation subscription if any
        if let Some(prev_mutation) = current_mutation.borrow_mut().take() {
            storage.remove_subscription(prev_mutation, scope_id);
        }

        // Store this new mutation
        current_mutation.borrow_mut().replace(mutation.clone());

        mutation
    }));

    // Remove mutation subscription on scope drop
    use_drop({
        move || {
            storage.remove_subscription(mutation.peek().clone(), scope_id);
        }
    });

    UseMutation { mutation }
}
