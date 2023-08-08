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

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryKeys {
    User(usize),
    Users,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryValue {
    UserName(String),
}

async fn fetch_user(id: usize) -> QueryResult<QueryValue, ()> {
    println!("Fetching user {id}...");
    sleep(Duration::from_millis(1000)).await;
    match id {
        0 => Ok(QueryValue::UserName("Marc".to_string())),
        1 => Ok(QueryValue::UserName("Evan".to_string())),
        _ => Err(()),
    }
    .into()
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
    let value = use_query(cx, move || vec![QueryKeys::User(*id), QueryKeys::Users], {
        move |keys| {
            Box::pin(async {
                if let Some(QueryKeys::User(id)) = keys.first() {
                    fetch_user(*id).await
                } else {
                    QueryResult::Err(())
                }
            })
        }
    });

    println!("Showing user {id}");

    let result: &QueryResult<QueryValue, ()> = &**value.result();

    render!( p { "{result:?}" } )
}

#[allow(non_snake_case)]
#[inline_props]
fn AnotherUser(cx: Scope, id: usize) -> Element {
    let value = use_query_config(cx, move || {
        QueryConfig::new(vec![QueryKeys::User(*id), QueryKeys::Users], move |keys| {
            Box::pin(async {
                if let Some(QueryKeys::User(id)) = keys.first() {
                    fetch_user(*id).await
                } else {
                    QueryResult::Err(())
                }
            })
        })
        .initial(|| Ok(QueryValue::UserName("Jonathan while loading".to_string())).into())
    });

    println!("Showing another user {id}");

    let result: &QueryResult<QueryValue, ()> = &**value.result();

    render!( p { "{result:?}" } )
}

fn app(cx: Scope) -> Element {
    let client = use_provide_query_client::<QueryValue, (), QueryKeys>(cx);

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
