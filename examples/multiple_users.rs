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

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryKeys {
    User(usize),
    Users,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryValue {
    UserName(String),
}

fn fetch_user(keys: &[QueryKeys]) -> BoxFuture<QueryResult<QueryValue, ()>> {
    Box::pin(async move {
        if let Some(QueryKeys::User(id)) = keys.first() {
            println!("Fetching user {id}");
            sleep(Duration::from_millis(1000)).await;
            match id {
                0 => Ok(QueryValue::UserName("Marc".to_string())),
                1 => Ok(QueryValue::UserName("Evan".to_string())),
                _ => Err(()),
            }
            .into()
        } else {
            QueryResult::Err(())
        }
    })
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
    let value = use_query(
        cx,
        || vec![QueryKeys::User(*id), QueryKeys::Users],
        fetch_user,
    );

    println!("Showing user {id}");

    render!( p { "{value.result().value():?}" } )
}

#[allow(non_snake_case)]
#[inline_props]
fn AnotherUser(cx: Scope, id: usize) -> Element {
    let value = use_query_config(cx, || {
        QueryConfig::new(vec![QueryKeys::User(*id), QueryKeys::Users], fetch_user)
            .initial(|| Ok(QueryValue::UserName("Jonathan while loading".to_string())).into())
    });

    println!("Showing another user {id}");

    render!( p { "{value.result().value():?}" } )
}

fn app(cx: Scope) -> Element {
    let client = use_query_client::<QueryValue, (), QueryKeys>(cx);

    let refresh_0 = {
        to_owned![client];
        move |_| {
            to_owned![client];
            cx.spawn(async move {
                client.invalidate_query(QueryKeys::User(0)).await;
            });
        }
    };

    let refresh_1 = {
        to_owned![client];
        move |_| {
            to_owned![client];
            cx.spawn(async move {
                client.invalidate_queries(&[QueryKeys::User(1)]).await;
            });
        }
    };

    let refresh_all = move |_| {
        to_owned![client];
        cx.spawn(async move {
            client.invalidate_query(QueryKeys::Users).await;
        });
    };

    render!(
        User { id: 0 }
        AnotherUser { id: 1 }
        button { onclick: refresh_0, label { "Refresh 0" } }
        button { onclick: refresh_1, label { "Refresh 1" } }
        button { onclick: refresh_all, label { "Refresh all" } }
    )
}
