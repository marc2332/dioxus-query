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

#[derive(Clone, PartialEq)]
enum QueryKeys {
    User(usize),
    Users,
}

async fn fetch_user(id: usize) -> QueryResult<String, ()> {
    println!("Fetching user {id}...");
    sleep(Duration::from_millis(1000)).await;
    match id {
        0 => Ok("Marc".to_string()),
        1 => Ok("Evan".to_string()),
        _ => Err(()),
    }
    .into()
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
    to_owned![id];

    let value = use_query(cx, move || vec![QueryKeys::User(id), QueryKeys::Users], {
        move |_keys| fetch_user(id)
    });

    let result = &*value.result();

    println!("User: {id} -> {result:?}");

    render!( p { "{result:?}" } )
}

#[allow(non_snake_case)]
#[inline_props]
fn AnotherUser(cx: Scope, id: usize) -> Element {
    to_owned![id];

    let value = use_query_config(cx, move || {
        let query_keys = vec![QueryKeys::User(id), QueryKeys::Users];
        let query_fn = move |_: &[QueryKeys]| fetch_user(id);
        let query_initial = || Ok("Jonathan while loading".to_string()).into();
        QueryConfig::new(query_keys, query_fn).initial(query_initial)
    });

    let result = &*value.result();

    println!("Another User: {id} -> {result:?}");

    render!( p { "{result:?}" } )
}

fn app(cx: Scope) -> Element {
    let client = use_provide_client(cx);

    let refresh_0 = |_| client.invalidate_query(QueryKeys::User(0));

    let refresh_1 = |_| client.invalidate_queries(&[QueryKeys::User(1)]);

    let refresh_all = |_| client.invalidate_query(QueryKeys::Users);

    render!(
        User { id: 0 }
        AnotherUser { id: 1 }
        button { onclick: refresh_0, label { "Refresh 0" } }
        button { onclick: refresh_1, label { "Refresh 1" } }
        button { onclick: refresh_all, label { "Refresh all" } }
    )
}
