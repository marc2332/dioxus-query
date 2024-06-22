#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use dioxus_query::prelude::*;
use std::time::Duration;
use tokio::time::sleep;

use dioxus::prelude::*;

fn main() {
    launch(app);
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum QueryKey {
    User(usize),
}

#[derive(PartialEq, Debug)]
enum QueryError {
    UserNotFound(usize),
    Unknown,
}

#[derive(PartialEq, Debug)]
enum QueryValue {
    UserName(String),
    UserAge(u8),
}

async fn fetch_user(keys: Vec<QueryKey>) -> QueryResult<QueryValue, QueryError> {
    if let Some(QueryKey::User(id)) = keys.first() {
        println!("Fetching name of user {id}");
        sleep(Duration::from_millis(650)).await;
        match id {
            0 => Ok(QueryValue::UserName("Marc".to_string())),
            _ => Err(QueryError::UserNotFound(*id)),
        }
        .into()
    } else {
        QueryResult::Err(QueryError::Unknown)
    }
}

async fn fetch_user_age(keys: Vec<QueryKey>) -> QueryResult<QueryValue, QueryError> {
    if let Some(QueryKey::User(id)) = keys.first() {
        println!("Fetching age of user {id}");
        sleep(Duration::from_millis(1000)).await;
        match id {
            0 => Ok(QueryValue::UserAge(0)),
            _ => Err(QueryError::UserNotFound(*id)),
        }
        .into()
    } else {
        QueryResult::Err(QueryError::Unknown)
    }
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let user_name = use_get_query([QueryKey::User(id)], fetch_user);
    let user_age = use_get_query([QueryKey::User(id)], fetch_user_age);

    println!("Rendering user {id}");

    rsx!(
        p { "{user_name.result().value():?}" }
        p { "{user_age.result().value():?}" }
    )
}

fn app() -> Element {
    use_init_query_client::<QueryValue, QueryError, QueryKey>();
    let client = use_query_client::<QueryValue, QueryError, QueryKey>();

    let refresh = move |_| client.invalidate_query(QueryKey::User(0));

    rsx!(
        User { id: 0 }
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
