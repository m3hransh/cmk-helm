---
title: Rust Iterators
tags: [concept]
status: completed
priority: 2
publish: true
aliases: [iterators, map, filter, collect, iterator adapters]
---

# Rust Iterators

Rust's iterator system is lazy and composable. Adapters like `.map()`, `.filter()`, and `.enumerate()` build a pipeline that produces values on demand. Nothing runs until you *consume* the iterator (e.g. with `.collect()` or a `for` loop).

---

## The Three Entry Points

```rust
let v = vec![1, 2, 3];

v.iter()        // borrows items: yields &i32
v.iter_mut()    // mutably borrows: yields &mut i32
v.into_iter()   // consumes the Vec: yields i32 (Vec is gone after)
```

In most cases you want `.iter()` — it borrows and you keep the original Vec.

---

## Common Adapters

**`.map()` — transform each item:**
```rust
let rows: Vec<Row> = version_groups
    .iter()
    .map(|g| Row::new([g.base.as_str(), g.latest_date.as_str()]))
    .collect();
```

**`.filter()` — keep only matching items:**
```rust
let stables: Vec<&Version> = versions
    .iter()
    .filter(|v| matches!(v.kind, VersionKind::StablePatch { .. }))
    .collect();
```

**`.enumerate()` — get index alongside item:**
```rust
for (i, group) in version_groups.iter().enumerate() {
    println!("Tab {i}: {}", group.base);
}
```

**`.flat_map()` — map then flatten nested iterators:**
```rust
// Turn Vec<VersionGroup> into flat Vec<&Version>
let all_versions: Vec<&Version> = groups
    .iter()
    .flat_map(|g| g.versions.iter())
    .collect();
```

**`.find()` — get first match:**
```rust
let selected = versions.iter().find(|v| v.base == "2.5.0");
// returns Option<&Version>
```

---

## Laziness — Why It Matters

The chain `.iter().map(...).filter(...)` creates no allocations until `.collect()` is called. Each item passes through the entire pipeline one at a time:

```rust
let result: Vec<String> = (0..1_000_000)
    .filter(|n| n % 2 == 0)   // only even numbers
    .map(|n| n.to_string())    // convert to String
    .take(5)                   // stop after 5
    .collect();                // now it runs — produces exactly 5 items
```

Without laziness, `.filter()` would allocate a 500,000-item Vec before `.take(5)` could limit it.

---

## `.collect()` — Consuming Into a Collection

`.collect()` is generic — the type annotation on the left tells it what to build:

```rust
let v: Vec<Row> = iter.map(|x| make_row(x)).collect();   // Vec
let s: HashSet<Edition> = iter.collect();                  // HashSet
let joined: String = chars.collect();                      // String from chars
```

---

## Iterators vs `for` Loops

```rust
// for loop style
let mut rows = Vec::new();
for v in &versions {
    rows.push(Row::new([v.base.as_str()]));
}

// iterator style (preferred — more expressive, easier to chain)
let rows: Vec<Row> = versions
    .iter()
    .map(|v| Row::new([v.base.as_str()]))
    .collect();
```

Both compile to the same code. Prefer the iterator style for transformations; prefer `for` loops for side effects.

---

## Metadata

**Tags:** concept
**Related:** [[Rust Closures]], [[Rust Ownership and Borrowing]], [[Rust Option Type]]
