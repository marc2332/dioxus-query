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
enum QueryKeys {
    User(usize),
    Users,
}

#[derive(PartialEq, Debug)]
enum QueryValue {
    UserName(String),
}

async fn fetch_user(keys: Vec<QueryKeys>) -> QueryResult<QueryValue, ()> {
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
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let value = use_simple_query([QueryKeys::User(id), QueryKeys::Users], fetch_user);

    println!("Showing user {id}");

    rsx!( p { "{value.result().value():?}" } )
}

#[allow(non_snake_case)]
#[component]
fn AnotherUser(id: usize) -> Element {
    let value = use_query(|| {
        let initial = QueryValue::UserName("Jonathan while loading".to_string()).into();

        Query::new([QueryKeys::User(id), QueryKeys::Users], fetch_user).initial(initial)
    });

    println!("Showing another user {id}");

    rsx!( p { "{value.result().value():?}" } )
}

fn app() -> Element {
    use_init_query_client::<QueryValue, (), QueryKeys>();
    let client = use_query_client::<QueryValue, (), QueryKeys>();

    let refresh_0 = move |_| {
        client.invalidate_query(QueryKeys::User(0));
    };

    let refresh_1 = move |_| client.invalidate_queries(&[QueryKeys::User(1)]);

    let refresh_all = move |_| client.invalidate_query(QueryKeys::Users);

    rsx!(
        User { id: 0 }
        AnotherUser { id: 1 }
        button { onclick: refresh_0, label { "Refresh 0" } }
        button { onclick: refresh_1, label { "Refresh 1" } }
        button { onclick: refresh_all, label { "Refresh all" } }
    )
}
