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
#[derive(Clone, PartialEq, Debug)]
enum QueryKeys {
    User(usize),
}

#[derive(Clone, PartialEq, Debug)]
enum QueryError {
    UserNotFound(usize),
}

async fn fetch_user(id: usize) -> QueryResult<String, QueryError> {
    sleep(Duration::from_millis(1000)).await;
    match id {
        0 => Ok("Marc".to_string()),
        _ => Err(QueryError::UserNotFound(id)),
    }
    .into()
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
    to_owned![id];

    let value = use_query(cx, move || vec![QueryKeys::User(id)], {
        move |_keys| fetch_user(id)
    });

    render!( p { "{value.result():?}" } )
}

fn app(cx: Scope) -> Element {
    let client = use_provide_client(cx);

    let refresh = |_| client.invalidate_query(QueryKeys::User(0));

    render!(
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
