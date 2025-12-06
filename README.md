[![Discord Server](https://img.shields.io/discord/1015005816094478347.svg?logo=discord&style=flat-square)](https://discord.gg/gwuU8vGRPr)

# dioxus-query 🦀⚡

**Fully-typed, async, reusable cached state management** for [Dioxus 🧬](https://dioxuslabs.com/). Inspired by [`TanStack Query`](https://tanstack.com/query/latest/docs/react/overview). 

See the [Docs](https://docs.rs/dioxus-query/latest/dioxus_query/) or join the [Discord](https://discord.gg/gwuU8vGRPr). 

## Support

- **Dioxus v0.7** 🧬
- Web, Desktop, and Blitz support

## Features
- [x] **Renderer-agnostic**
- [x] **Queries** and **Mutations**
- [x] **Fully typed**, no type erasing
- [x] Invalidate queries **manually**
- [x] Invalidate queries on **equality change**
- [x] **Concurrent execution** of queries
- [x] **Background interval re-execution** of queries
- [x] **Opt-in in-memory cache** of queries results
- [x] Works with ReactiveContext-powered hooks like **`use_effect` or `use_memo`**
- [ ] On window/tab focus invalidation


## Installation

Install the latest release:
```bash
cargo add dioxus-query
```

## Example

Run manually:
```bash	
cargo run --example hello_world
```

Code:
```rust
#[derive(Clone)]
struct FancyClient;

impl FancyClient {
    pub fn name(&self) -> &'static str {
        "Marc"
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

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
    let user_name = use_query(Query::new(id, GetUserName(Captured(FancyClient))));

    rsx!(
        p { "{user_name.read().state():?}" }
    )
}

fn app() -> Element {
    let refresh = move |_| async move {
        QueriesStorage::<GetUserName>::invalidate_matching(0).await;
    };

    rsx!(
        User { id: 0 }
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
```

## To Do
- Tests
- Improved documentation
- Real-world examples

MIT License
