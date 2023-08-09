#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use dioxus_query::*;
use futures_util::future::BoxFuture;
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
    Unknown,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryValue {
    UserName(String),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum MutationValue {
    UserUpdated(usize),
}

fn fetch_user(keys: &[QueryKeys]) -> BoxFuture<QueryResult<QueryValue, QueryError>> {
    Box::pin(async move {
        if let Some(QueryKeys::User(id)) = keys.first() {
            println!("Fetching user {id}");
            sleep(Duration::from_millis(1000)).await;
            match id {
                0 => Ok(QueryValue::UserName("Marc".to_string())),
                _ => Err(QueryError::UserNotFound(*id)),
            }
            .into()
        } else {
            QueryResult::Err(QueryError::Unknown)
        }
    })
}

fn update_user(
    (id, _name): (usize, String),
) -> BoxFuture<'static, MutationResult<MutationValue, QueryError>> {
    Box::pin(async move { Ok(MutationValue::UserUpdated(id)).into() })
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
    let value = use_query(cx, move || vec![QueryKeys::User(*id)], fetch_user);
    let mutate = use_mutation(cx, update_user);

    let onclick = |_| {
        to_owned![mutate];
        cx.spawn(async move {
            mutate.mutate((0, "Not Marc".to_string())).await;
        });
    };

    println!("Showing user {id}");

    render!(
       p { "{value.result().value():?}" }
       button {
           onclick: onclick,
           "change to random"
       }
    )
}

fn app(cx: Scope) -> Element {
    let client = use_provide_query_client::<QueryValue, QueryError, QueryKeys>(cx);

    let refresh = |_| {
        to_owned![client];
        cx.spawn(async move {
            client.invalidate_query(QueryKeys::User(0)).await;
        });
    };

    render!(
        User { id: 0 }
        User { id: 0 }
        User { id: 0 }
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
