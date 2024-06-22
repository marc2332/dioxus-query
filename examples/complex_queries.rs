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
    Users,
}

#[derive(PartialEq, Debug)]
enum QueryValue {
    UserName(String),
}

async fn fetch_user(keys: Vec<QueryKey>) -> QueryResult<QueryValue, ()> {
    if let Some(QueryKey::User(id)) = keys.first() {
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
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let value = use_get_query([QueryKey::User(id), QueryKey::Users], fetch_user);

    println!("Rendering user {id}");

    rsx!( p { "{value.result().value():?}" } )
}

#[allow(non_snake_case)]
#[component]
fn AnotherUser(id: usize) -> Element {
    let value = use_query([QueryKey::User(id), QueryKey::Users], || {
        let initial = QueryValue::UserName("Jonathan while loading".to_string()).into();

        Query::new(fetch_user).initial(initial)
    });

    println!("Rendering another user {id}");

    rsx!( p { "{value.result().value():?}" } )
}

fn app() -> Element {
    use_init_query_client::<QueryValue, (), QueryKey>();
    let client = use_query_client::<QueryValue, (), QueryKey>();

    let refresh_0 = move |_| {
        client.invalidate_query(QueryKey::User(0));
    };

    let refresh_1 = move |_| client.invalidate_queries(&[QueryKey::User(1)]);

    let refresh_all = move |_| client.invalidate_query(QueryKey::Users);

    rsx!(
        User { id: 0 }
        AnotherUser { id: 1 }
        button { onclick: refresh_0, label { "Refresh 0" } }
        button { onclick: refresh_1, label { "Refresh 1" } }
        button { onclick: refresh_all, label { "Refresh all" } }
    )
}
