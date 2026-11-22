//! Shared Slint glue for Rimelight's native apps: plain-Rust rules (toast queue, color parsing, table sorting and
//! selection, tree rows, display helpers, select and menu rules, theme types) and the `macro_rules!` that an app expands
//! in its own crate to wire those rules to the Slint types it generates.
//!
//! The layering rule (AGENTS.md, layering 2) applies here: nothing in this crate names a type generated from markup.
//! The Slint side is written as macros the app expands:
//!
//! - [`wire_select!`] and [`wire_menu!`] emit the conversions and the `SelectNav` and `MenuNav` callbacks.
//! - [`wire_color_hex!`] emits the `ColorHex` callback.
//! - [`wire_theme!`] emits the theme setters for the `Tokens` global.
//! - [`toast`] needs no macro: the app's window code calls the queue and sets its own model with
//!   [`toast::sync_model`].
//!
//! This crate depends on `slint` only, and only for the model helpers and the `Color` type.

pub mod color;
pub mod display;
pub mod menu;
pub mod select;
pub mod table;
pub mod theme;
pub mod toast;
pub mod tree;
