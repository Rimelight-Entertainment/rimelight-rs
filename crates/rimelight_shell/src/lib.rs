//! Rimelight app shell. Two parts.
//!
//! The markup: Slint for the parts every Rimelight app frame shares. The icon rail, the sidebar, the status bar and its
//! jobs menu, the phone tab bar and more drawer, the split pane, the command palette, the quick action button, the
//! shortcuts dialog, the connection state, and the About screen with Slint's `AboutSlint` widget, which the Slint
//! royalty-free licence requires on an About screen reachable from the top-level menu. An app compiles it by mapping three
//! library paths in its `build.rs`: `rimelight_shell` to [`UI_DIR`], `rimelight_ui` to `rimelight_ui::UI_DIR` and
//! `rimelight_design` to `rimelight_design::UI_DIR`, then importing, for example, `@rimelight_shell/components/rail.slint`.
//!
//! The page templates (ST5.4) are in `ui/templates/`: `@rimelight_shell/templates/list-page.slint` (`ListPage`, the T-LIST
//! shape), `board-page.slint` (`BoardPage`, the T-BOARD shape) and `detail-page.slint` (`DetailPage`, the T-DETAIL shape). They
//! are compositions of `rimelight_ui` components, so they live here with the rest of the page chrome, not in
//! `rimelight_ui_kit`, which holds only plain Rust. Their state is in `rimelight_ui_kit` (`page_state`, `list`, `board`).
//!
//! The plain-Rust models: the page tabs ([`tabs`]), their back and forward [`history`], the tile groups ([`tiles`]), the
//! tab drop zones ([`tile_drop`]), and the top-level workspaces and their saving ([`workspace`]). They are generic over
//! the app's [`Routes`], which give the route titles, the Home route and the rail areas.
//!
//! The shared rules: the palette filter and highlight step ([`palette`]) and the rail badge text ([`badge`]). The row
//! builders that read an app's route manifest stay in each app, since each app has its own manifest.

pub mod badge;
pub mod history;
pub mod palette;
pub mod routes;
pub mod tabs;
pub mod tile_drop;
pub mod tiles;
pub mod workspace;

pub use routes::Routes;

/// The folder holding this crate's `.slint` files, for an app's Slint library paths.
pub const UI_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui");
