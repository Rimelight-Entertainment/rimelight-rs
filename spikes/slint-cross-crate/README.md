# Slint cross-crate spike

Proves that a library crate can ship Slint markup plus plain Rust glue, and that only the app crate compiles Slint.

## What it proves

- `spike_ui` ships `ui/tokens.slint` and `ui/toast.slint` and has no `slint` dependency and no `build.rs`. It exports
  `UI_DIR`, the folder the app's build script adds as the library `@spike_ui`.
- `spike_kit` depends on the `slint` runtime only, with no `slint-build`. Its `ToastQueue` holds the toast list and
  the auto-dismiss timers. It names no type generated from markup. It reaches the app through the `ToastSink` trait.
- `spike_app` is the only crate that compiles Slint. Its `build.rs` imports `@spike_ui/...` and compiles
  `ui/main.slint`. `src/lib.rs` calls `slint::include_modules!()`.
- Two ways to connect the queue to the app:
  - Macro: `spike_kit::wire_toasts!(AppWindow, ToastState, ToastEntry);` generates `WindowToastSink` in the app crate.
  - Hand-written trait impl: `src/manual.rs`, built with `--features manual-trait`.
- A headless test (`crates/spike_app/tests/toast.rs`) pushes a toast, checks that `ToastState.entries` has one row,
  calls `dismiss` through the global, and checks that it has none.

## Layout

```
crates/spike_ui    markup only (ui/tokens.slint, ui/toast.slint) and UI_DIR
crates/spike_kit   ToastQueue, ToastSink, wire_toasts! macro
crates/spike_app   build.rs, ui/main.slint, lib.rs (include_modules), main.rs, manual.rs, tests/toast.rs
```

## Build and test

Run from this folder. The toolchain is pinned in `rust-toolchain.toml` (1.100.0).

```
cargo build                          # macro path (default)
cargo test                           # headless test, macro path
cargo build --features manual-trait  # hand-written ToastSink path
cargo test --features manual-trait   # the same test against the hand-written sink
```

The app's `build.rs` reruns when `ui/` or `spike_ui`'s `ui/` folder changes.

## Open questions

- Can the same `.slint` library be imported by two app crates? The spike has one app. Two apps would each run their
  own `build.rs` with the same `UI_DIR` library path. Expected to work, not yet tried.
- Incremental rebuild: when a library `.slint` file changes, does `slint-build` emit `rerun-if-changed` for every
  imported file? The spike adds `rerun-if-changed={UI_DIR}` by hand. Check whether a plain `cargo build` after editing
  `spike_ui/ui/toast.slint` recompiles `spike_app`.
- Re-exports: `export { ToastStack, ToastState, ToastEntry, Tokens }` in `main.slint` must cause codegen of the
  struct and the global in Rust. Not yet confirmed by a build.
- Ownership cycle: the queue holds the sink, and the sink holds the Slint callback. The dismiss handler holds only a
  weak reference to the queue, and timers hold weak references too, so no cycle is expected. Not measured.
- Timer drop inside its own callback: `dismiss` removes a toast's timer from the map, and a timer's callback calls
  `dismiss`, so a timer can be dropped while it fires. Expected to be safe; unverified.
- Naming: `wire_toasts!` generates a fixed name (`WindowToastSink`). Two invocations in one module would clash.
- The `slint` dependency must stay in the app crate: the macro expansion names `::slint` paths.
