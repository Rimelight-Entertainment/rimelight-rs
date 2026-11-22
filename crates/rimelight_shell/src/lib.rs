//! Rimelight app shell: Slint markup for the parts every Rimelight app frame shares. The icon rail, the sidebar, the
//! status bar and its jobs menu, the phone tab bar and more drawer, the split pane, the command palette, the quick
//! action button, the shortcuts dialog, the connection state, and the About screen with Slint's `AboutSlint` widget,
//! which the Slint royalty-free licence requires on an About screen reachable from the top-level menu.
//!
//! Markup only (ST2.5a). An app compiles it by mapping three library paths in its `build.rs`: `rimelight_shell` to
//! [`UI_DIR`], `rimelight_ui` to `rimelight_ui::UI_DIR` and `rimelight_design` to `rimelight_design::UI_DIR`, then
//! importing, for example, `@rimelight_shell/components/rail.slint`. The Rust models that fill these components (rows,
//! tiles, palette entries) still live in each app until the `ShellModel` trait exists (ST2.5b).

/// The folder holding this crate's `.slint` files, for an app's Slint library paths.
pub const UI_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui");
