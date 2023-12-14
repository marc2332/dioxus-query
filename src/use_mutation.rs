use dioxus::prelude::*;
use futures_util::Future;
use std::{fmt::Debug, rc::Rc, sync::Arc};

pub type MutationFn<T, E, A> = dyn Fn(A) -> Box<dyn Future<Output = MutationResult<T, E>>>;

/// A query mutation.
#[derive(Clone)]
pub struct UseMutation<T, E, A> {
    value: Rc<RefCell<MutationResult<T, E>>>,
    mutation_fn: Arc<Box<MutationFn<T, E, A>>>,
    scheduler: Arc<dyn Fn(ScopeId)>,
    scope_id: ScopeId,
}

impl<T, E, A> UseMutation<T, E, A>
where
    T: Clone,
    E: Clone,
{
    /// Get the current result from the query mutation.
    pub fn result(&self) -> Ref<'_, MutationResult<T, E>> {
        self.value.borrow()
    }

    async fn inner_mutate(
        arg: A,
        value: &Rc<RefCell<MutationResult<T, E>>>,
        scheduler: &Arc<dyn Fn(ScopeId)>,
        scope_id: ScopeId,
        mutation_fn: &Arc<Box<MutationFn<T, E, A>>>,
    ) {
        let cached_value = value.borrow().clone().into();

        // Set state to loading and notify
        *value.borrow_mut() = MutationResult::Loading(cached_value);

        // TODO optimization: Check if the value was already loading
        // to decide to call the scheduler or not
        (scheduler)(scope_id);

        // Trigger the mutation function
        let fut = (mutation_fn)(arg);
        let fut = Box::into_pin(fut);
        let new_value = fut.await;

        // Set state to the new value and notify
        *value.borrow_mut() = new_value;

        // TODO optimization: Check if the previous and new value are
        // different to decide to call the scheduler or not
        (scheduler)(scope_id);
    }

    async fn inner_silent_mutate(
        arg: A,
        value: &Rc<RefCell<MutationResult<T, E>>>,
        mutation_fn: &Arc<Box<MutationFn<T, E, A>>>,
    ) {
        let cached_value = value.borrow().clone().into();

        // Set state to loading
        *value.borrow_mut() = MutationResult::Loading(cached_value);

        // Trigger the mutation function
        let fut = (mutation_fn)(arg);
        let fut = Box::into_pin(fut);
        let new_value = fut.await;

        // Set state to the new value
        *value.borrow_mut() = new_value;
    }

    /// Call the mutation function with a set of arguments, in the **background**.
    pub fn mutate(&self, arg: A)
    where
        T: 'static,
        E: 'static,
        A: 'static,
    {
        let value = self.value.clone();
        let scheduler = self.scheduler.clone();
        let scope_id = self.scope_id;
        let mutation_fn = self.mutation_fn.clone();
        spawn(
            async move { Self::inner_mutate(arg, &value, &scheduler, scope_id, &mutation_fn).await },
        );
    }

    /// Call the mutation function with a set of arguments.
    pub async fn manual_mutate(&self, arg: A) {
        Self::inner_mutate(
            arg,
            &self.value,
            &self.scheduler,
            self.scope_id,
            &self.mutation_fn,
        )
        .await;
    }

    /// Call the mutation function silently with a set of arguments, in the **background**.
    /// This will not make the component re run.
    pub async fn mutate_silent(&self, arg: A)
    where
        T: 'static,
        E: 'static,
        A: 'static,
    {
        let value = self.value.clone();
        let mutation_fn = self.mutation_fn.clone();
        spawn(async move {
            Self::inner_silent_mutate(arg, &value, &mutation_fn).await;
        });
    }

    /// Call the mutation function silently with a set of arguments.
    /// This will not make the component re run.
    pub async fn manual_mutate_silent(&self, arg: A) {
        Self::inner_silent_mutate(arg, &self.value, &self.mutation_fn).await;
    }
}

/// The result of a mutation.
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

/// Create mutation. See [UseMutation] on how to use it.
pub fn use_mutation<T, E, A, M, F>(cx: &ScopeState, mutation_fn: M) -> &UseMutation<T, E, A>
where
    T: 'static + PartialEq,
    E: 'static + PartialEq,
    A: 'static,
    M: Fn(A) -> F + 'static,
    F: Future<Output = MutationResult<T, E>> + 'static,
{
    cx.use_hook(|| UseMutation {
        value: Rc::new(RefCell::new(MutationResult::Pending)),
        mutation_fn: Arc::new(Box::new(move |p| {
            let fut = mutation_fn(p);
            Box::new(fut)
        })),
        scheduler: cx.schedule_update_any(),
        scope_id: cx.scope_id(),
    })
}
