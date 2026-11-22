//! Markup for the spike's toast stack. This crate ships `.slint` files only; the app crate
//! compiles them (see `crates/spike_app/build.rs`). Nothing here names a type generated from markup.

/// Folder holding the `.slint` files. The app's build script adds it as the library `@spike_ui`.
pub const UI_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui");
