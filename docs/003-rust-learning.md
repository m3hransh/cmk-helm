---
id: "003"
title: Rust Learning — Concepts Encountered
tags: [rust, learning, ownership, async, tui]
created: 2025-04-03
---

# Rust Learning — Concepts Encountered

← [[000-index]]

This note grows as new Rust patterns appear in the codebase. Each concept links to where it shows up in the code.

---

## Ownership and the Borrow Checker

Rust's most distinctive feature. Every value has exactly one *owner*. When the owner goes out of scope, the value is dropped (freed).

```rust
let s = String::from("hello");
let t = s;          // ownership MOVED to t
println!("{}", s);  // ← compile error: s was moved
```

### References (`&T` and `&mut T`)

Instead of moving, you can *borrow*:

```rust
fn greet(name: &str) { println!("Hello, {name}!"); }

let s = String::from("world");
greet(&s);          // borrow — s still owned here
println!("{}", s);  // fine
```

**In this project:** `api::fetch_packages(base_url: &str)` takes a `&str` (borrowed string slice) so the caller keeps ownership of the URL string.

### `&mut self` vs `mut self` vs `self`

In `impl App`:
- `&self` — read the struct, don't change it
- `&mut self` — read AND modify the struct (event handlers, `select_next`)
- `self` — *consume* the struct (used in `run()` — the `App` is moved into the loop and dropped when it returns)

---

## Enums: Richer Than You Think

Rust enums can carry data — each variant is like a mini-struct:

```rust
enum Screen {
    PackageList,
    Configure { selected: Package, site_input: String },
    Installing { config: InstallConfig },
}
```

This is **not** like a C enum. It's more like a tagged union / algebraic data type. Pattern matching with `match` is exhaustive — the compiler errors if you forget a variant.

```rust
match self.screen {
    Screen::PackageList => { /* draw table */ }
    Screen::Configure { selected, site_input } => { /* draw form */ }
    Screen::Installing { config } => { /* draw progress */ }
    // no `_` needed — all variants are covered
}
```

**Why this matters:** You cannot accidentally access `site_input` when you're on the `Installing` screen. The type system makes the invalid state unrepresentable.

---

## `Option<T>` — No Null Pointers

Rust has no `null`. Instead, "might not exist" is modelled as `Option<T>`:

```rust
enum Option<T> {
    Some(T),  // has a value
    None,     // nothing here
}
```

`TableState::selected()` returns `Option<usize>`. We handle it with `.map()` or `if let`:

```rust
let prev = self.table_state.selected()
    .map(|i| i.saturating_sub(1))  // transform if Some
    .unwrap_or(0);                 // default if None
```

---

## `Result<T, E>` and the `?` Operator

Functions that can fail return `Result<T, E>`:

```rust
enum Result<T, E> {
    Ok(T),   // success with a value
    Err(E),  // failure with an error
}
```

The `?` operator short-circuits on error — it's syntactic sugar for:

```rust
let x = some_fallible_call()?;
// expands to:
let x = match some_fallible_call() {
    Ok(v) => v,
    Err(e) => return Err(e.into()),
};
```

`anyhow::Result<T>` is `Result<T, anyhow::Error>`. `anyhow::Error` can hold any error type and builds a chain of context messages:

```rust
let html = client.get(url)
    .send().await
    .with_context(|| format!("Failed to reach {url}"))?;
//  ^ adds context before propagating the underlying error
```

---

## The TUI Event Loop

Ratatui apps follow a tight draw → handle input → repeat loop:

```rust
while !self.should_quit {
    terminal.draw(|frame| self.render(frame))?;  // 1. draw
    self.handle_events()?;                        // 2. handle input
}
```

**Why `event::poll(16ms)`?**
`poll` blocks until an event arrives OR 16ms elapses (≈ 60fps). This avoids 100% CPU usage while keeping the UI responsive. Without the timeout, `event::read()` would block forever if the user isn't typing.

**Raw mode** (`ratatui::init()`): disables line buffering, so keypresses arrive immediately without needing Enter. Also switches to the *alternate screen buffer* so the normal terminal is preserved and restored on exit.

---

## Closures

A closure is an anonymous function that can *capture* variables from its surrounding scope:

```rust
terminal.draw(|frame| self.render(frame))?;
//            ^^^^^^^^^^^^^^^^^^^^^^^
//            closure — captures &mut self
```

Rust closures are statically typed and zero-cost. The `|arg| body` syntax is Rust's equivalent of Python lambdas, but they can span multiple lines and capture by reference or value.

---

## Iterators and `.map().collect()`

Rust's iterator adapters are lazy and composable:

```rust
let rows: Vec<Row> = self.packages
    .iter()                           // creates an iterator
    .map(|p| Row::new([...]))         // transforms each item
    .collect();                       // consumes iterator into Vec<Row>
```

- `.iter()` — borrows items (`&Package`)
- `.into_iter()` — consumes items (moves `Package`)
- `.iter_mut()` — mutably borrows items

`.collect()` is generic — the return type annotation `Vec<Row>` tells it what to build.

---

## `#[derive(...)]` — Free Trait Implementations

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package { ... }
```

The compiler auto-generates:
- `Debug` — `format!("{:?}", pkg)` works
- `Clone` — `pkg.clone()` makes a deep copy
- `PartialEq`, `Eq` — `pkg1 == pkg2` works (field-by-field comparison)

Use `cargo expand` (included in the dev shell) to see exactly what code `#[derive]` generates — very educational.

---

## `String` vs `&str`

| Type | What it is |
|------|-----------|
| `String` | Owned, heap-allocated, growable UTF-8 string |
| `&str` | Borrowed reference to a UTF-8 string slice (could be in a String, literal, etc.) |

- Function arguments that just need to *read* a string: use `&str`
- Storing a string in a struct: use `String`
- Converting: `.to_string()` or `.into()` on a `&str` gives you a `String`

`"hello"` is a `&'static str` — a string literal baked into the binary.

---

## `saturating_sub` — Arithmetic Without Underflow

```rust
i.saturating_sub(1)  // never goes below 0 for usize
```

In Rust, integer overflow/underflow **panics** in debug builds (it's a bug). `saturating_sub` clamps at the minimum value instead. Equivalent to `max(0, i - 1)` but without needing a cast.

---

## `if let ... else { return }` — Guarded Enum Access

When a method can be called on multiple enum variants but only does work for one, use:

```rust
fn render_configure(&self, frame: &mut Frame, area: Rect) {
    let Screen::Configure { version, edition, site_input } = &self.screen else { return };
    // now `version`, `edition`, `site_input` are in scope
}
```

The `let ... else { ... }` block is a "let-else" statement (stable since Rust 1.65). The `else` branch must diverge (`return`, `break`, `panic!`). This avoids deeply nested `if let` or `match` arms when you only care about one variant.

---

## `#[tokio::test]` — Async Tests

The standard `#[test]` macro doesn't understand `async`. For async test functions, use:

```rust
#[tokio::test]
async fn my_test() {
    let result = some_async_fn().await;
    assert!(result.is_ok());
}
```

`#[tokio::test]` sets up a single-threaded tokio runtime scoped to the test function. It's the async equivalent of `#[test]` — nothing else changes.

---

## `#[ignore]` — Opt-in Tests

Tests that require external resources (network, credentials, running services) are marked `#[ignore]`:

```rust
#[tokio::test]
#[ignore = "requires ~/.cmk-credentials and network access"]
async fn integration_test() { ... }
```

- `cargo test` — skips ignored tests
- `cargo test -- --ignored` — runs ONLY ignored tests
- `cargo test -- --include-ignored` — runs all tests

The string after `=` appears in test output and explains *why* the test is ignored.

---

## `matches!` Macro — Pattern Matching in Assertions

```rust
assert!(matches!(
    &versions[0].kind,
    VersionKind::Daily { date } if date == "2026.04.03"
));
```

`matches!(expr, pattern)` returns `true` if `expr` matches `pattern`. You can add a guard clause with `if`. It's cleaner than `match expr { pattern => true, _ => false }` in test assertions.

---

## To Explore Next

- [ ] Lifetimes (`'a`) — when the borrow checker needs explicit annotations
- [ ] Traits — Rust's alternative to interfaces/type classes
- [ ] `Arc<Mutex<T>>` — sharing state across async tasks
- [ ] `tokio::spawn` — running tasks truly in parallel
- [ ] `cargo test` — writing and running unit and integration tests
