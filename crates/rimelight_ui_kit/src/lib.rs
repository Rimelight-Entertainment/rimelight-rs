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
//! - [`wire_page_state!`] emits `load_state`, which maps a [`page_state::PageState`] to the `PageLoadState` enum.
//! - [`toast`] needs no macro: the app's window code calls the queue and sets its own model with
//!   [`toast::sync_model`].
//!
//! The page templates (ST5.4) keep their state here: [`page_state`] (loading, empty, error, ready), [`list`] (the filter,
//! sort and selection of a list page) and [`board`] (the columns and card moves of a board page). The markup for the
//! templates lives in `rimelight_shell` (`ui/templates/`), since it composes `rimelight_ui` components.
//!
//! This crate depends on `slint` only, and only for the model helpers and the `Color` type.

pub mod board;
pub mod clipboard;
pub mod color;
pub mod display;
pub mod list;
pub mod menu;
pub mod page_state;
pub mod select;
pub mod table;
pub mod theme;
pub mod toast;
pub mod tree;
