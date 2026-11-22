//! App-side glue. This crate is the only one that compiles Slint: `slint::include_modules!()` pulls in the
//! generated `AppWindow`, `ToastState` and `ToastEntry`.
//!
//! It is a library as well as the binary (`src/main.rs`) so that `tests/toast.rs` can reach the generated types.

slint::include_modules!();

// Default: the macro generates `WindowToastSink`. With `--features manual-trait` the hand-written sink is used instead.
#[cfg(not(feature = "manual-trait"))]
spike_kit::wire_toasts!(AppWindow, ToastState, ToastEntry);

#[cfg(feature = "manual-trait")]
pub mod manual;
