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

#[derive(Clone, PartialEq, Hash, Eq)]
struct GetUserName;

impl QueryCapability for GetUserName {
    type Ok = String;
    type Err = ();
    type Keys = usize;

    async fn run(&self, user_id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        println!("Fetching name of user {user_id}");
        sleep(Duration::from_millis(650)).await;
        match user_id {
            0 => Ok("Marc".to_string()),
            _ => Err(()),
        }
    }
}

#[derive(Clone, PartialEq, Hash, Eq)]
struct GetUserAge;

impl QueryCapability for GetUserAge {
    type Ok = usize;
    type Err = ();
    type Keys = usize;

    async fn run(&self, user_id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        println!("Fetching age of user {user_id}");
        sleep(Duration::from_millis(1000)).await;
        match user_id {
            0 => Ok(0),
            _ => Err(()),
        }
    }
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let user_name = use_query(Query::new(id, GetUserName));
    let user_age = use_query(Query::new(id, GetUserAge).stale_time(Duration::from_secs(4)));

    println!("Rendering user {id}");

    rsx!(
        p { "{user_name.read().state():?}" }
        p { "{user_age.read().state():?}" }
    )
}

fn app() -> Element {
    let refresh = move |_| async move {
        QueriesStorage::<GetUserName>::invalidate_matching(0).await;
        QueriesStorage::<GetUserAge>::invalidate_matching(0).await;
    };

    rsx!(
        User { id: 0 }
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
