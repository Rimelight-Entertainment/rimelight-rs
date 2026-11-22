//! Rimelight component library: Slint markup for the shared UI, plus the bundled fonts as bytes.
//!
//! This crate is markup only. It has no Slint dependency at run time (Slint is a dev-dependency, for the ui test binary)
//! and no `build.rs`. Nothing here names a type generated from
//! the markup, so Rust code in this crate never touches generated Slint types.
//!
//! What is in the crate:
//! - `ui/components/`: 81 exported components (buttons, inputs, menus, dialogs, toasts, tables and more).
//! - `ui/typography.slint` (the `Type` global) and `ui/icons.slint` (the `Icons` global, Lucide icons from `assets/icons`).
//! - `ui/fonts/`: Noto Sans and JetBrains Mono (SIL OFL 1.1). `typography.slint` registers them, so any app that uses
//!   the library gets the faces without importing them itself.
//!
//! How an app uses it. Only the app crate compiles Slint. Its `build.rs` adds two library paths and compiles its own
//! entry file:
//!
//! ```ignore
//! let libraries = HashMap::from([
//!     ("rimelight_ui".to_owned(), PathBuf::from(rimelight_ui::UI_DIR)),
//!     ("rimelight_design".to_owned(), PathBuf::from(rimelight_design::UI_DIR)),
//! ]);
//! let config = slint_build::CompilerConfiguration::new().with_library_paths(libraries);
//! slint_build::compile_with_config("ui/main.slint", config)?;
//! ```
//!
//! The app's markup then imports from `@rimelight_ui/components/button.slint`, and tokens come from
//! `@rimelight_design/tokens.slint`. The library path `rimelight_design` is supplied by the `rimelight_design` crate
//! (its own `UI_DIR`), not by this crate.
//!
//! Facade rule. Feature code imports these components from `rimelight_ui`, not the Slint std-widgets. The components
//! are the styled versions of the std-widgets, and the std-widgets carry Fluent colours that the tokens do not control.
//!
//! No literal colours. Every colour comes from the `Tokens` global in `rimelight_design`, so the light and dark themes
//! and the high-contrast palette change in one place.

/// Absolute path of the library's `.slint` markup folder, for an app's `build.rs` to register as the library path
/// `rimelight_ui`.
pub const UI_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui");

/// Bundled fonts (P0.14). Noto Sans for the UI and JetBrains Mono for code. Both are SIL OFL 1.1.
///
/// The files live in `ui/fonts/`, with versions, URLs and SHA-256 hashes in `ui/fonts/README.md`. The Slint markup
/// imports the same files (`ui/typography.slint`), so the app registers them there.
pub mod fonts {
    /// Family name of the UI face, as stored in the font files.
    pub const UI_FAMILY: &str = "Noto Sans";
    /// Family name of the monospace face, as stored in the font files.
    pub const MONO_FAMILY: &str = "JetBrains Mono";

    pub const NOTO_SANS_REGULAR: &[u8] =
        include_bytes!("../ui/fonts/noto-sans/NotoSans-Regular.ttf");
    pub const NOTO_SANS_MEDIUM: &[u8] = include_bytes!("../ui/fonts/noto-sans/NotoSans-Medium.ttf");
    pub const NOTO_SANS_SEMIBOLD: &[u8] =
        include_bytes!("../ui/fonts/noto-sans/NotoSans-SemiBold.ttf");
    pub const NOTO_SANS_BOLD: &[u8] = include_bytes!("../ui/fonts/noto-sans/NotoSans-Bold.ttf");
    pub const NOTO_SANS_ITALIC: &[u8] = include_bytes!("../ui/fonts/noto-sans/NotoSans-Italic.ttf");
    pub const JETBRAINS_MONO_REGULAR: &[u8] =
        include_bytes!("../ui/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf");
    pub const JETBRAINS_MONO_BOLD: &[u8] =
        include_bytes!("../ui/fonts/jetbrains-mono/JetBrainsMono-Bold.ttf");

    /// Every bundled face, for tests and for apps that want to list them.
    pub const FACES: &[(&str, &str, &[u8])] = &[
        ("Noto Sans", "Noto Sans Regular", NOTO_SANS_REGULAR),
        ("Noto Sans", "Noto Sans Medium", NOTO_SANS_MEDIUM),
        ("Noto Sans", "Noto Sans SemiBold", NOTO_SANS_SEMIBOLD),
        ("Noto Sans", "Noto Sans Bold", NOTO_SANS_BOLD),
        ("Noto Sans", "Noto Sans Italic", NOTO_SANS_ITALIC),
        (
            "JetBrains Mono",
            "JetBrains Mono Regular",
            JETBRAINS_MONO_REGULAR,
        ),
        ("JetBrains Mono", "JetBrains Mono Bold", JETBRAINS_MONO_BOLD),
    ];

    /// Characters every bundled face must cover: Portuguese letters (both cases), currency and quote marks, dashes,
    /// ellipsis, and printable ASCII. `R$` is covered by `R` and `$` from ASCII.
    pub const REQUIRED_CHARS: &str = "ãõçáéíóúâêôàÃÕÇÁÉÍÓÚÂÊÔÀ€£“”‘’–—…";

    #[cfg(test)]
    mod tests {
        use super::{FACES, REQUIRED_CHARS};

        fn missing_in(bytes: &[u8], label: &str) -> Vec<char> {
            let face = ttf_parser::Face::parse(bytes, 0)
                .unwrap_or_else(|e| panic!("{label} is not a readable font: {e}"));
            REQUIRED_CHARS
                .chars()
                .chain((0x20u8..=0x7e).map(char::from))
                .filter(|c| face.glyph_index(*c).is_none_or(|g| g.0 == 0))
                .collect()
        }

        #[test]
        fn every_bundled_face_covers_portuguese_currency_quotes_and_ascii() {
            let failures: Vec<String> = FACES
                .iter()
                .filter_map(|(family, label, bytes)| {
                    let missing = missing_in(bytes, label);
                    (!missing.is_empty())
                        .then(|| format!("{family} / {label} is missing {missing:?}"))
                })
                .collect();
            assert!(
                failures.is_empty(),
                "glyph coverage failed:
{}",
                failures.join(
                    "
"
                )
            );
        }

        #[test]
        fn family_names_in_the_files_match_the_constants() {
            for (family, label, bytes) in FACES {
                let face = ttf_parser::Face::parse(bytes, 0).expect("parse");
                let found = face
                    .names()
                    .into_iter()
                    .filter(|n| {
                        n.name_id == ttf_parser::name_id::TYPOGRAPHIC_FAMILY
                            || n.name_id == ttf_parser::name_id::FAMILY
                    })
                    .filter_map(|n| n.to_string())
                    .any(|name| name == *family);
                assert!(found, "{label} does not name itself {family}");
            }
        }

        #[test]
        fn bundled_files_are_truetype_or_opentype() {
            for (_, label, bytes) in FACES {
                let magic = &bytes[..4];
                assert!(
                    magic == [0x00, 0x01, 0x00, 0x00] || magic == *b"OTTO",
                    "{label} does not start with a TrueType or OpenType signature: {magic:02x?}"
                );
            }
        }
    }
}

/// The icon pair (ST2.7): `ui/icons.slint` and the Lucide files in `assets/icons`. These tests read the folder relative to
/// this crate, so they run from any checkout. The Rust side of the icon names (`IconName`) is in space_ui and checks
/// against `ui/icons.slint` there.
#[cfg(test)]
mod icon_files {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn icons_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/icons")
    }

    /// Every `@image-url("../assets/icons/<stem>.svg")` in `ui/icons.slint`, as file stems (`house`, `chart-column`, ...).
    fn icon_stems_in_markup() -> BTreeSet<String> {
        let text = std::fs::read_to_string(PathBuf::from(super::UI_DIR).join("icons.slint"))
            .expect("read ui/icons.slint");
        text.split("@image-url(\"../assets/icons/")
            .skip(1)
            .map(|rest| {
                let end = rest.find(".svg\")").expect("icon path ends in .svg\")");
                rest[..end].to_string()
            })
            .collect()
    }

    #[test]
    fn every_icon_named_by_the_markup_has_a_file() {
        let stems = icon_stems_in_markup();
        assert!(!stems.is_empty(), "ui/icons.slint names no icons");
        for stem in stems {
            let path = icons_dir().join(format!("{stem}.svg"));
            assert!(path.is_file(), "{stem} has no file at {}", path.display());
        }
    }

    #[test]
    fn the_icon_licence_is_bundled() {
        let licence = std::fs::read_to_string(icons_dir().join("LICENSE")).expect("icon LICENSE");
        assert!(
            licence.starts_with("ISC License"),
            "icons are ISC (Lucide); the LICENSE file must be bundled"
        );
    }
}
