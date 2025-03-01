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

#[derive(Debug)]
enum MutationError {}

#[derive(PartialEq, Debug)]
enum MutationValue {
    UserUpdated(usize),
}

async fn update_user((id, _name): (usize, String)) -> MutationResult<MutationValue, MutationError> {
    println!("Mutating user");
    sleep(Duration::from_millis(1000)).await;
    Ok(MutationValue::UserUpdated(id))
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let mutate = use_mutation(update_user);

    let onclick = move |_| {
        mutate.mutate((id, "Not Marc".to_string()));
    };

    println!("Rendering user {id}");

    rsx!(
        p { "{*mutate.result():?}" }
        button {
            onclick,
            if mutate.result().is_loading() {
              "Loading..."
           } else {
               "Load"
           }
        }
    )
}

fn app() -> Element {
    rsx!(
        User { id: 0 }
        User { id: 1 }
        User { id: 2 }
        User { id: 3 }
    )
}
