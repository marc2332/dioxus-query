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

#[derive(Clone)]
struct FancyClient;

impl FancyClient {
    pub fn name(&self) -> &'static str {
        "Marc"
    }

    pub fn age(&self) -> u8 {
        123
    }
}

#[derive(Query)]
#[query(ok = String, err = (), key = usize)]
struct GetUserName(Captured<FancyClient>);

impl GetUserName {
    async fn run(&self, user_id: &usize) -> Result<String, ()> {
        println!("Fetching name of user {user_id}");
        sleep(Duration::from_millis(650)).await;
        match user_id {
            0 => Ok(self.0.name().to_string()),
            _ => Err(()),
        }
    }
}

#[derive(Query)]
#[query(ok = u8, err = (), key = usize)]
struct GetUserAge(Captured<FancyClient>);

impl GetUserAge {
    async fn run(&self, user_id: &usize) -> Result<u8, ()> {
        println!("Fetching age of user {user_id}");
        sleep(Duration::from_millis(1000)).await;
        match user_id {
            0 => Ok(self.0.age()),
            _ => Err(()),
        }
    }
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let fancy_client = FancyClient;

    let user_name = use_query(
        Query::new(id, GetUserName(Captured(fancy_client.clone())))
            .stale_time(Duration::from_secs(5))
            .interval_time(Duration::from_secs(15)),
    );
    let user_age = use_query(
        Query::new(id, GetUserAge(Captured(fancy_client))).stale_time(Duration::from_secs(10)),
    );

    println!("Rendering user {id}");

    rsx!(
        p { "{user_name.read().state():?}" }
        p { "{user_age.read().state():?}" }
    )
}

fn app() -> Element {
    let mut replicas = use_signal(|| 1);

    let refresh = move |_| async move {
        QueriesStorage::<GetUserName>::invalidate_matching(0).await;
        QueriesStorage::<GetUserAge>::invalidate_matching(0).await;
    };

    let new_replica = move |_| async move {
        replicas += 1;
    };

    rsx!(
        button { onclick: new_replica,
            label { "New replica" }
        }
        button { onclick: refresh,
            label { "Refresh" }
        }
        for i in 0..replicas() {
            User { key: "{i}", id: 0 }
        }
    )
}
