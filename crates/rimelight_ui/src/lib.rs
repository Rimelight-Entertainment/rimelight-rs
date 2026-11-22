//! Rimelight Slint component library. Markup lives in `ui/` and is filled in ST2.3; this crate
//! carries no Slint dependency. Apps compile the markup through `UI_DIR` as an `@rimelight_ui` library path.

/// Absolute path of the library's `.slint` markup folder, for an app's `build.rs` to register as a library path.
pub const UI_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui");
