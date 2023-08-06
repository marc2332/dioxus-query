#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use dioxus_query::*;
use std::time::Duration;
use tokio::time::sleep;

use dioxus::prelude::*;

fn main() {
    dioxus_desktop::launch(app);
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum QueryKeys {
    User(usize),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryError {
    UserNotFound(usize),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryValue {
    UserName(String),
}

async fn fetch_user(id: usize) -> QueryResult<QueryValue, QueryError> {
    println!("Fetching user {id}");
    sleep(Duration::from_millis(1000)).await;
    match id {
        0 => Ok(QueryValue::UserName("Marc".to_string())),
        _ => Err(QueryError::UserNotFound(id)),
    }
    .into()
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
    to_owned![id];

    let value = use_query(cx, move || vec![QueryKeys::User(id)], {
        move |_keys| Box::pin(fetch_user(id))
    });

    println!("Showing user {id}");

    let result: &QueryResult<QueryValue, QueryError> = &**value.result();

    render!( p { "{result:?}" } )
}

fn app(cx: Scope) -> Element {
    let client = use_provide_client::<QueryValue, QueryError, QueryKeys>(cx);

    let refresh = |_| client.invalidate_query(QueryKeys::User(0));

    render!(
        User { id: 0 }
        User { id: 0 }
        User { id: 0 }
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
