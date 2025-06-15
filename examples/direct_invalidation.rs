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

#[derive(Query)]
#[query(ok = SystemTime, err = (), key = "()")]
struct GetTime;

impl GetTime {
    async fn run(&self, _: &()) -> Result<SystemTime, ()> {
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
