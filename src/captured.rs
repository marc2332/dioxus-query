use std::{hash::Hash, ops::Deref};

/// Capture values to use later inside Mutations, but with a catch, if the capture value changes the mutation wont recapture it because
/// the [PartialEq] implementation always returns false.
///
/// So in other words `Capture(1) == Capture(1)` will be `false`.
///
/// **This is intended to use for value types that are not mean to be diffed and that are expected to maintain their value across time.
/// Like "Clients" of external resources.**
#[derive(Clone)]
pub struct Captured<T: Clone>(pub T);

impl<T: Clone> Hash for Captured<T> {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

impl<T: Clone> PartialEq for Captured<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T: Clone> Eq for Captured<T> {}

impl<T: Clone> Deref for Captured<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
