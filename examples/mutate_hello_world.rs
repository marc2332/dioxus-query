#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use dioxus_query::prelude::*;
use std::{cell::RefCell, rc::Rc, time::Duration};
use tokio::time::sleep;

use dioxus::prelude::*;

fn main() {
    launch(app);
}

#[derive(Clone, Default)]
struct FancyClient(Rc<RefCell<i32>>);

impl PartialEq for FancyClient {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for FancyClient {}

impl FancyClient {
    pub fn age(&self) -> i32 {
        *self.0.borrow()
    }

    pub fn set_age(&self, new_age: i32) {
        *self.0.borrow_mut() = new_age;
    }
}

// NEW: Most ergonomic derive syntax!
#[derive(Query)] // Clone, PartialEq, Eq, Hash are derived by Query
#[query(ok = i32, err = (), key = usize)]
struct GetUserAge {
    client: Captured<FancyClient>,
}

impl GetUserAge {
    async fn run(&self, user_id: &usize) -> Result<i32, ()> {
        println!("Fetching age of user {user_id}");
        sleep(Duration::from_millis(1000)).await;
        match user_id {
            0 => Ok(self.client.age()), // Corrected: No .0 needed due to Deref on Captured
            _ => Err(()),
        }
    }
}

#[derive(Mutation)]
#[mutation(ok = i32, err = (), key = usize)]
struct SetUserAge {
    client: Captured<FancyClient>,
}

// User still defines the run logic and any specific lifecycle methods
impl SetUserAge {
    async fn run(&self, user_id: &usize) -> Result<i32, ()> {
        println!("Updating age of user {user_id}");
        sleep(Duration::from_millis(400)).await;
        let curr_age = self.client.age();
        self.client.set_age(curr_age + 1);
        match user_id {
            0 => Ok(self.client.age()),
            _ => Err(()),
        }
    }

    async fn on_settled(&self, user_id: &usize, _result: &Result<i32, ()>) {
        QueriesStorage::<GetUserAge>::invalidate_matching(*user_id).await;
    }
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let fancy_client = use_context::<FancyClient>();

    let user_age = use_query(
        Query::new(
            id,
            GetUserAge {
                client: Captured(fancy_client.clone()),
            },
        )
        .stale_time(Duration::from_secs(4)),
    );

    println!("Rendering user {id}");

    rsx!(
        p { "{user_age.read().state():?}" }
    )
}

fn app() -> Element {
    let fancy_client = use_context_provider(FancyClient::default);

    let set_user_age = use_mutation(Mutation::new(SetUserAge {
        client: Captured(fancy_client.clone()),
    }));

    let increase_age = move |_| async move {
        set_user_age.mutate_async(0).await;
    };

    rsx!(
        User { id: 0 }
        User { id: 0 }
        button { onclick: increase_age,
            label { "Increse age" }
        }
    )
}
