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

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let user_name = use_query(Query::new(id, GetUserName(Captured(FancyClient)))).suspend()?;

    println!("Rendering user {id}");

    rsx!(
        p { "{user_name:?}" }
    )
}

fn app() -> Element {
    let refresh = move |_| async move {
        QueriesStorage::<GetUserName>::invalidate_matching(0).await;
    };

    rsx!(
        SuspenseBoundary {
            fallback: |_| rsx!(
                p {
                    "Loading user..."
                }
            ),
            User { id: 0 }
        }
        button { onclick: refresh, label { "Refresh" } }
    )
}
