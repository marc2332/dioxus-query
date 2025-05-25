#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use dioxus_query::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

use dioxus::prelude::*;

fn main() {
    launch(app);
}

#[derive(Clone, PartialEq, Hash, Eq)]
struct GetTime;

impl QueryCapability for GetTime {
    type Ok = SystemTime;
    type Err = ();
    type Keys = ();

    async fn run(&self, _: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        Ok(SystemTime::now())
    }
}

fn app() -> Element {
    let time = use_query(Query::new((), GetTime));

    let refresh = move |_| {
        time.invalidate();
    };

    let time = time
        .read()
        .state()
        .ok()
        .map(|time| time.duration_since(UNIX_EPOCH));

    rsx!(
        p {
            "{time:?}"
        }
        button { onclick: refresh, label { "Refresh" } }
    )
}
