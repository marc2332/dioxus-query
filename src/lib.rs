//! # dioxus-query
//! **Fully-typed, async, reusable state management and synchronization** for [Dioxus 🧬](https://dioxuslabs.com/).
//! ## Usage
//!
//! ```rust
//! #[derive(Clone, PartialEq, Eq, Hash)]
//! enum QueryKeys {
//!     User(usize),
//! }
//!
//! #[derive(Clone, PartialEq, Eq, Hash, Debug)]
//! enum QueryError {
//!     UserNotFound(usize),
//!     Unknown
//! }
//!
//! #[derive(Clone, PartialEq, Eq, Hash, Debug)]
//! enum QueryValue {
//!     UserName(String),
//! }
//!
//! async fn fetch_user(keys: Vec<QueryKeys>) -> QueryResult<QueryValue, QueryError> {
//!     if let Some(QueryKeys::User(id)) = keys.first() {
//!         println!("Fetching user {id}");
//!         sleep(Duration::from_millis(1000)).await;
//!         match id {
//!             0 => Ok(QueryValue::UserName("Marc".to_string())),
//!             _ => Err(QueryError::UserNotFound(*id)),
//!         }
//!         .into()
//!     } else {
//!         QueryResult::Err(QueryError::Unknown)
//!     }
//! }
//!
//! #[allow(non_snake_case)]
//! #[inline_props]
//! fn User(cx: Scope, id: usize) -> Element {
//!    let value = use_query(cx, || vec![QueryKeys::User(*id)], fetch_user);
//!
//!     render!( p { "{value.result().value():?}" } )
//! }
//!
//!
//! fn app(cx: Scope) -> Element {
//!     let client = use_query_client::<QueryValue, QueryError, QueryKeys>(cx);
//!
//!     let refresh = move |_| {
//!         to_owned![client];
//!         cx.spawn(async move {
//!             client.invalidate_query(QueryKeys::User(0)).await;
//!         });
//!     };
//!
//!     render!(
//!         User { id: 0 }
//!         button { onclick: refresh, label { "Refresh" } }
//!     )
//! }
//! ```
//!

mod cached_result;
mod result;
mod use_mutation;
mod use_query;
mod use_query_client;

pub mod prelude {
    pub use crate::cached_result::*;
    pub use crate::result::*;
    pub use crate::use_mutation::*;
    pub use crate::use_query::*;
    pub use crate::use_query_client::*;
    pub use futures_util;
}
