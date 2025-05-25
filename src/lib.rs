#![doc = include_str!("../README.md")]

pub mod captured;
pub mod mutation;
pub mod query;

pub mod prelude {
    pub use crate::captured::*;
    pub use crate::mutation::*;
    pub use crate::query::*;
}
