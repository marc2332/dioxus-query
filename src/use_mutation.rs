use dioxus_core::*;
use dioxus_hooks::*;
pub use futures_util;
use futures_util::future::BoxFuture;
use std::{fmt::Debug, rc::Rc, sync::Arc};

pub type MutationFn<T, E, P> = dyn Fn(P) -> BoxFuture<'static, MutationResult<T, E>>;

/// A query mutation.
#[derive(Clone)]
pub struct UseMutation<T, E, P> {
    value: Rc<RefCell<MutationResult<T, E>>>,
    mutation_fn: Arc<Box<MutationFn<T, E, P>>>,
    scheduler: Arc<dyn Fn(ScopeId) + Send + Sync>,
    scope_id: ScopeId,
}

impl<T: Clone, E: Clone, P> UseMutation<T, E, P> {
    /// Get the current result from the query mutation.
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

/// Create mutation. See [UseMutation] on how to use it.
pub fn use_mutation<T, E, P>(
    cx: &ScopeState,
    mutation_fn: impl Fn(P) -> BoxFuture<'static, MutationResult<T, E>> + 'static,
) -> &UseMutation<T, E, P>
where
    T: 'static + PartialEq,
    E: 'static + PartialEq,
    P: 'static,
{
    cx.use_hook(|| UseMutation {
        value: Rc::new(RefCell::new(MutationResult::Pending)),
        mutation_fn: Arc::new(Box::new(mutation_fn)),
        scheduler: cx.schedule_update_any(),
        scope_id: cx.scope_id(),
    })
}
