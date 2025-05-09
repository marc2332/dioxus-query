use dioxus_lib::prelude::*;
use futures_util::Future;
use std::{fmt::Debug, mem, sync::Arc};

pub type MutationFn<T, E, A> = dyn Fn(A) -> Box<dyn Future<Output = MutationResult<T, E>>>;

/// A query mutation.
pub struct UseMutation<T, E, A>
where
    T: 'static,
    E: 'static,
    A: 'static,
{
    value: Signal<MutationState<T, E>>,
    mutation_fn: Signal<Arc<Box<MutationFn<T, E, A>>>>,
    scheduler: Signal<Arc<dyn Fn(ScopeId)>>,
    scope_id: ScopeId,
}

impl<T, E, A> Clone for UseMutation<T, E, A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, E, A> Copy for UseMutation<T, E, A> {}

impl<T, E, A> UseMutation<T, E, A>
where
    T: 'static,
    E: 'static,
{
    /// Get the current result from the query mutation.
    pub fn result(&self) -> ReadableRef<Signal<MutationState<T, E>>> {
        self.value.peek()
    }

    async fn inner_mutate(
        arg: A,
        mut value: Signal<MutationState<T, E>>,
        scheduler: Signal<Arc<dyn Fn(ScopeId)>>,
        scope_id: ScopeId,
        mutation_fn: Signal<Arc<Box<MutationFn<T, E, A>>>>,
    ) {
        // Set state to loading and notify
        value.write().set_loading();

        // TODO optimization: Check if the value was already loading
        // to decide to call the scheduler or not
        (scheduler.peek())(scope_id);

        // Run the mutation function
        let fut = (mutation_fn.peek())(arg);
        let fut = Box::into_pin(fut);
        let new_value = fut.await;

        // Set state to the new value and notify
        value.set(new_value.into());

        // TODO optimization: Check if the previous and new value are
        // different to decide to call the scheduler or not
        (scheduler.peek())(scope_id);
    }

    async fn inner_silent_mutate(
        arg: A,
        mut value: Signal<MutationState<T, E>>,
        mutation_fn: Signal<Arc<Box<MutationFn<T, E, A>>>>,
    ) {
        // Set state to loading
        value.write().set_loading();

        // Run the mutation function
        let fut = (mutation_fn.peek())(arg);
        let fut = Box::into_pin(fut);
        let new_value = fut.await;

        // Set state to the new value
        value.set(new_value.into());
    }

    /// Call the mutation function with a set of arguments, runs in the **background**.
    pub fn mutate(&self, arg: A)
    where
        T: 'static,
        E: 'static,
        A: 'static,
    {
        let value = self.value;
        let scheduler = self.scheduler;
        let scope_id = self.scope_id;
        let mutation_fn = self.mutation_fn;
        spawn(
            async move { Self::inner_mutate(arg, value, scheduler, scope_id, mutation_fn).await },
        );
    }

    /// Call the mutation function with a set of arguments and await it in place.
    pub async fn mutate_async(&self, arg: A) {
        Self::inner_mutate(
            arg,
            self.value,
            self.scheduler,
            self.scope_id,
            self.mutation_fn,
        )
        .await;
    }

    /// Call the mutation function silently with a set of arguments, in the **background**.
    /// This will not make the component re run.
    pub fn mutate_silent(&self, arg: A)
    where
        T: 'static,
        E: 'static,
        A: 'static,
    {
        let value = self.value;
        let mutation_fn = self.mutation_fn;
        spawn(async move {
            Self::inner_silent_mutate(arg, value, mutation_fn).await;
        });
    }

    /// Call the mutation function silently with a set of arguments.
    /// This will not make the component re run.
    pub async fn manual_mutate_silent(&self, arg: A) {
        Self::inner_silent_mutate(arg, self.value, self.mutation_fn).await;
    }
}

/// The result of a mutation.
pub type MutationResult<T, E> = Result<T, E>;

/// The state of a mutation.
#[derive(PartialEq, Debug)]
pub enum MutationState<T, E> {
    /// Contains a successful or errored result
    Settled(MutationResult<T, E>),
    /// Contains a loading state that may or not have a cached result
    Loading(Option<T>),
    /// Mutation has not been triggered yet
    Pending,
}

impl<T, E> MutationState<T, E> {
    pub fn is_ok(&self) -> bool {
        matches!(self, MutationState::Settled(Ok(..)))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, MutationState::Settled(Ok(..)))
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, MutationState::Loading(..))
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, MutationState::Pending)
    }

    pub fn set_loading(&mut self) {
        let result = mem::replace(self, Self::Loading(None)).into();
        if let Some(v) = result {
            *self = Self::Loading(Some(v))
        }
    }
}

impl<T, E> From<Result<T, E>> for MutationState<T, E> {
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(v) => MutationState::Settled(Ok(v)),
            Err(e) => MutationState::Settled(Err(e)),
        }
    }
}

impl<T, E> From<MutationState<T, E>> for Option<T> {
    fn from(result: MutationState<T, E>) -> Self {
        match result {
            MutationState::Settled(Ok(v)) => Some(v),
            MutationState::Settled(Err(_)) => None,
            MutationState::Loading(v) => v,
            MutationState::Pending => None,
        }
    }
}

/// Create mutation. See [UseMutation] on how to use it.
pub fn use_mutation<T, E, A, M, F>(mutation_fn: M) -> UseMutation<T, E, A>
where
    T: 'static + PartialEq,
    E: 'static,
    A: 'static,
    M: Fn(A) -> F + 'static,
    F: Future<Output = MutationResult<T, E>> + 'static,
{
    use_hook(|| UseMutation {
        value: Signal::new(MutationState::Pending),
        mutation_fn: Signal::new(Arc::new(Box::new(move |p| {
            let fut = mutation_fn(p);
            Box::new(fut)
        }))),
        scheduler: Signal::new(schedule_update_any()),
        scope_id: current_scope_id().unwrap(),
    })
}
