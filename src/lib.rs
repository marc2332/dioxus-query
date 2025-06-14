#![doc = include_str!("../README.md")]

pub mod captured;
pub mod mutation;
pub mod query;

// Re-export the derive macro
pub use dioxus_query_macro::Mutation;
pub use dioxus_query_macro::Query;

pub mod prelude {
    pub use crate::captured::*;
    pub use crate::mutation::*;
    pub use crate::query::*;
    // Re-export the derive macro in prelude too
    pub use dioxus_query_macro::Mutation;
    pub use dioxus_query_macro::Query;
}
