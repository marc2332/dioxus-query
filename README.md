[![Discord Server](https://img.shields.io/discord/1015005816094478347.svg?logo=discord&style=flat-square)](https://discord.gg/gwuU8vGRPr)

# dioxus-query 🦀⚡

**Fully-typed, async, reusable state management and synchronization** for [Dioxus 🧬](https://dioxuslabs.com/). Inspired by [`TanStack Query`](https://tanstack.com/query/latest/docs/react/overview). 

See the [Docs](https://docs.rs/dioxus-query/latest/dioxus_query/) or join the [Discord](https://discord.gg/gwuU8vGRPr). 

⚠️ **Work in progress ⚠️**

## Support

- **Dioxus v0.4** 🧬
- All renderer ([web](https://dioxuslabs.com/learn/0.4/getting_started/wasm), [desktop](https://dioxuslabs.com/learn/0.4/getting_started/desktop), [freya](https://github.com/marc2332/freya), etc)
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
enum QueryKeys {
    User(usize),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryError {
    UserNotFound(usize),
    Unknown
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum QueryValue {
    UserName(String),
}

async fn fetch_user(keys: Vec<QueryKeys>) -> QueryResult<QueryValue, QueryError> {
    if let Some(QueryKeys::User(id)) = keys.first() {
        println!("Fetching user {id}");
        sleep(Duration::from_millis(1000)).await;
        match id {
            0 => Ok(QueryValue::UserName("Marc".to_string())),
            _ => Err(QueryError::UserNotFound(*id)),
        }
        .into()
    } else {
        QueryResult::Err(QueryError::Unknown)
    }
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
   let value = use_query(cx, || vec![QueryKeys::User(*id)], fetch_user);

    render!( p { "{value.result().value():?}" } )
}

fn app(cx: Scope) -> Element {
    let client = use_query_client::<QueryValue, QueryError, QueryKeys>(cx);

    let refresh = move |_| {
        to_owned![client];
        cx.spawn(async move {
            client.invalidate_query(QueryKeys::User(0)).await;
        });
    };

    render!(
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
```

## Features
- [x] Renderer-agnostic
- [x] Typed Query keys, errors and results
- [x] Manual query/queries invalidation
- [ ] Automatic/smart query invalidation
- [ ] Query aborting
- [x] Global Query + Function caching
- [x] Concurrent queries and mutations

## To Do
- Tests
- Documentation
- Real-world examples
- Clean up code

MIT License
