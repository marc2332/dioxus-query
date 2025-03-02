[![Discord Server](https://img.shields.io/discord/1015005816094478347.svg?logo=discord&style=flat-square)](https://discord.gg/gwuU8vGRPr)

# dioxus-query 🦀⚡

**Fully-typed, async, reusable cached state management** for [Dioxus 🧬](https://dioxuslabs.com/). Inspired by [`TanStack Query`](https://tanstack.com/query/latest/docs/react/overview). 

See the [Docs](https://docs.rs/dioxus-query/latest/dioxus_query/) or join the [Discord](https://discord.gg/gwuU8vGRPr). 

## Support

- **Dioxus v0.6** 🧬
- All renderers ([web](https://dioxuslabs.com/learn/0.4/getting_started/wasm), [desktop](https://dioxuslabs.com/learn/0.4/getting_started/desktop), [freya](https://github.com/marc2332/freya), etc)
- Both WASM and native targets

## Installation

Install the latest release:
```bash
cargo add dioxus-query
```

## Example

```bash	
cargo run --example simple
```

## Usage

```rust
#[derive(Clone, PartialEq, Eq, Hash)]
enum QueryKey {
    User(usize),
}

#[derive(Debug)]
enum QueryError {
    UserNotFound(usize),
    Unknown
}

#[derive(PartialEq, Debug)]
enum QueryValue {
    UserName(String),
}

async fn fetch_user(keys: Vec<QueryKey>) -> QueryResult<QueryValue, QueryError> {
    if let Some(QueryKey::User(id)) = keys.first() {
        println!("Fetching user {id}");
        sleep(Duration::from_millis(1000)).await;
        match id {
            0 => Ok(QueryValue::UserName("Marc".to_string())),
            _ => Err(QueryError::UserNotFound(*id)),
        }
    } else {
        Err(QueryError::Unknown)
    }
}

#[allow(non_snake_case)]
#[component]
fn User(id: usize) -> Element {
   let value = use_get_query([QueryKey::User(id)], fetch_user);

    rsx!( p { "{value.result().value():?}" } )
}

fn app() -> Element {
    let client = use_init_query_client::<QueryValue, QueryError, QueryKey>();

    let onclick = move |_| {
         client.invalidate_queries(&[QueryKey::User(0)]);
    };

    rsx!(
        User { id: 0 }
        button { onclick, label { "Refresh" } }
    )
}
```

## Features
- [x] Renderer-agnostic
- [x] Queries and mutations
- [x] Typed Mutations, Query keys, Errors and Values
- [x] Invalidate queries manually
- [x] Invalidate queries when keys change
- [x] Concurrent and batching of queries
- [x] Concurrent mutations
- [ ] Background interval invalidation
- [ ] On window focus invalidation


## To Do
- Tests
- Documentation
- Real-world examples

MIT License
