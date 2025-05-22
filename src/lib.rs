//! # dioxus-query
//! **Fully-typed, async, reusable state management and synchronization** for [Dioxus 🧬](https://dioxuslabs.com/).
//! ## Usage
//!
//! ```rust
//! #[derive(Clone, PartialEq, Eq, Hash)]
//! enum QueryKey {
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
//! async fn fetch_user(keys: Vec<QueryKey>) -> QueryResult<QueryValue, QueryError> {
//!     if let Some(QueryKey::User(id)) = keys.first() {
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
//! #[component]
//! fn User(id: usize) -> Element {
//!    let value = use_get_query(QueryKey::User(id)], fetch_user);
//!
//!     rsx!( p { "{value.result().value():?}" } )
//! }
//!
//!
//! fn app() -> Element {
//!     let client = use_query_client::<QueryValue, QueryError, QueryKey>();
//!
//!     let onclick = move |_| {
//!         client.invalidate_query(QueryKey::User(0));
//!     };
//!
//!     rsx!(
//!         User { id: 0 }
//!         button { onclick, label { "Refresh" } }
//!     )
//! }
//! ```
//!

mod captured;
pub mod mutation;
pub mod query;

pub mod prelude {
    pub use crate::captured::*;
    pub use crate::mutation::*;
    pub use crate::query::*;
}
