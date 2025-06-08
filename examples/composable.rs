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

#[derive(Clone, PartialEq, Hash, Eq)]
struct GetUserName(Captured<FancyClient>);

impl QueryCapability for GetUserName {
    type Ok = String;
    type Err = ();
    type Keys = usize;

    async fn run(&self, user_id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        println!("Fetching name of user {user_id}");
        sleep(Duration::from_millis(650)).await;
        match user_id {
            0 => Ok(self.0.name().to_string()),
            _ => Err(()),
        }
    }
}

#[derive(Clone, PartialEq, Hash, Eq)]
struct GetUserInfo(Captured<FancyClient>);

impl QueryCapability for GetUserInfo {
    type Ok = (String, u8);
    type Err = ();
    type Keys = usize;

    async fn run(&self, user_id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let name = QueriesStorage::get(
            GetQuery::new(*user_id, GetUserName(self.0.clone()))
                .stale_time(Duration::from_secs(30))
                .clean_time(Duration::from_secs(30)),
        )
        .await;
        let name = name.as_settled().clone()?;
        println!("Fetching age of user {user_id}");
        sleep(Duration::from_millis(1000)).await;
        match user_id {
            0 => Ok((name, self.0.age())),
            _ => Err(()),
        }
    }
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let fancy_client = FancyClient;

    let user_info = use_query(
        Query::new(id, GetUserInfo(Captured(fancy_client))).stale_time(Duration::from_secs(10)),
    );

    println!("Rendering user {id}");

    rsx!(
        p { "{user_info.read().state():?}" }
    )
}

fn app() -> Element {
    let refresh = move |_| async move {
        QueriesStorage::<GetUserInfo>::invalidate_matching(0).await;
    };

    rsx!(
        User { id: 0 }
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
