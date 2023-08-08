# dioxus-query 🦀⚡

**Fully-typed, async, reusable state management and synchronization** for [Dioxus 🧬](https://dioxuslabs.com/). Inspired by [`TanStack Query`](https://tanstack.com/query/latest/docs/react/overview).

⚠️ **Work in progress ⚠️**

## Installation

Compatible with **Dioxus v0.4**.

```bash
cargo add dioxus-query
```

## Example

```bash	
cargo run --example basic
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

async fn fetch_user(id: usize) -> QueryResult<QueryValue, QueryError> {
    sleep(Duration::from_millis(1000)).await;
    match id {
        0 => Ok(QueryValue::UserName("Marc".to_string())),
        _ => Err(QueryError::UserNotFound(id)),
    }
    .into()
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
    to_owned![id];

    let value = use_query(cx, move || vec![QueryKeys::User(id)], {
        move |keys| Box::pin(async {
            if let Some(QueryKeys::User(id)) = keys.first() {
                fetch_user(*id).await
            } else {
                QueryResult::Err(QueryError::Unknown)
            }
        })
    });

    let result: &QueryResult<QueryValue, QueryError> = &**value.result();

    render!( p { "{result:?}" } )
}

fn app(cx: Scope) -> Element {
    let client = use_provide_query_client::<QueryValue, QueryError, QueryKeys>(cx);

    let refresh = |_| client.invalidate_query(QueryKeys::User(0));

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
- [x] Global Query caching
- [x] Concurrent queries when dispatched as groups
- [x] Sequencial queries when dispatched individually

## To Do
- Tests
- Documentation
- Real-world examples
- Clean up code

MIT License