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
#[derive(Clone, PartialEq, Debug)]
enum QueryKeys {
    User(usize),
}

#[derive(Clone, PartialEq, Debug)]
enum QueryError {
    UserNotFound(usize)
}

async fn fetch_user(id: usize) -> QueryResult<String, QueryError> {
    sleep(Duration::from_millis(1000)).await;
    match id {
        0 => Ok("Marc".to_string()),
        _ => Err(QueryError::UserNotFound(id)),
    }
    .into()
}

#[allow(non_snake_case)]
#[inline_props]
fn User(cx: Scope, id: usize) -> Element {
    to_owned![id];

    let value = use_query(cx, move || vec![QueryKeys::User(id)], {
        move |_keys| fetch_user(id)
    });

    render!( p { "{value.result():?}" } )
}

fn app(cx: Scope) -> Element {
    let client = use_provide_client(cx);

    let refresh = |_| client.invalidate_query(QueryKeys::User(0));

    render!(
        User { id: 0 }
        button { onclick: refresh, label { "Refresh" } }
    )
}
```

## Features
- [x] Renderer-agnostic
- [x] Typed Query keys
- [x] Manual query invalidation
- [ ] Automatic/smart query invalidation
- [ ] Query aborting
- [ ] Global Query caching

MIT License