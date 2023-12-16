#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use dioxus_query::prelude::*;
use std::time::Duration;
use tokio::time::sleep;

use dioxus::prelude::*;

fn main() {
    dioxus_desktop::launch(app);
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum MutationError {}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum MutationValue {
    UserUpdated(usize),
}

async fn update_user((id, _name): (usize, String)) -> MutationResult<MutationValue, MutationError> {
    println!("Mutating user");
    sleep(Duration::from_millis(1000)).await;
    Ok(MutationValue::UserUpdated(id)).into()
}

#[allow(non_snake_case)]
#[component]
fn User(cx: Scope, id: usize) -> Element {
    let mutate = use_mutation(cx, update_user);

    let onclick = |_| {
        mutate.mutate((0, "Not Marc".to_string()));
    };

    println!("Showing user {id}");

    render!(
        p { "{*mutate.result():?}" }
        button { onclick: onclick,
            if mutate.result().is_loading() {
              "Loading..."
           } else {
               "Load"
           }
        }
    )
}

fn app(cx: Scope) -> Element {
    render!(
        User { id: 0 }
        User { id: 0 }
        User { id: 0 }
        User { id: 0 }
    )
}
