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

#[derive(Query)] // Added Query derive and other necessary derives
#[query(ok = String, err = (), key = usize)] // Added query attribute
struct GetUserName;

// User still defines the run logic
impl GetUserName {
    async fn run(&self, user_id: &usize) -> Result<String, ()> {
        println!("Fetching name of user {user_id}");
        sleep(Duration::from_millis(650)).await;
        match user_id {
            0 => Ok("Marc".to_string()),
            _ => Err(()),
        }
    }
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let user_name = use_query(
        Query::new(id, GetUserName)
            .stale_time(Duration::MAX)
            .interval_time(Duration::from_secs(2)),
    );

    println!("Rendering user {id}");

    rsx!(
        p { "{user_name.read().state():?}" }
    )
}

fn app() -> Element {
    let mut replicas = use_signal(|| 1);

    let new_replica = move |_| async move {
        replicas += 1;
    };

    rsx!(
        button { onclick: new_replica,
            label { "New replica" }
        }
        for i in 0..replicas() {
            User { key: "{i}", id: 0 }
        }
    )
}
