//! The theme mode, the text scale and the reduced-motion flag of the Slint tokens (decision 0007, S1). The types here are
//! plain Rust. The app expands [`wire_theme!`] in its own crate, where the generated `Tokens` global is in scope, to get
//! the setters that write into it.
//!
//! The mode is a property of the `Tokens` global, not a Rust-side copy. Under `System` the Slint expression for
//! `Tokens.dark` reads `Palette.color-scheme`, and the Slint window backend updates that when the OS theme changes
//! (winit `ThemeChanged` calls `set_color_scheme`). So switching OS theme repaints without any Rust call. The mode and
//! the flags below are set from Rust.

/// What the user chose: light, dark, or system (which follows the OS through `Palette.color-scheme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    Light,
    Dark,
    #[default]
    System,
}

/// The text-scale setting. Multiplies every type size and line height, through `Tokens.text-scale-percent` in Slint.
/// The Slint side owns the multiplication (`Type` and `Tokens.text-factor`); this is the Rust choice of step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextScale {
    #[default]
    P100,
    P125,
    P150,
    P200,
}

impl TextScale {
    pub const ALL: [TextScale; 4] = [Self::P100, Self::P125, Self::P150, Self::P200];

    /// The multiplier: 1.0, 1.25, 1.5 or 2.0.
    pub fn factor(self) -> f32 {
        match self {
            Self::P100 => 1.0,
            Self::P125 => 1.25,
            Self::P150 => 1.5,
            Self::P200 => 2.0,
        }
    }

    /// The percentage shown to the user and used in file names: 100, 125, 150 or 200.
    pub fn percent(self) -> u32 {
        match self {
            Self::P100 => 100,
            Self::P125 => 125,
            Self::P150 => 150,
            Self::P200 => 200,
        }
    }

    /// Parses a percentage (100, 125, 150 or 200). Other values are rejected.
    pub fn from_percent(percent: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.percent() == percent)
    }
}

/// Emits, in the calling module, the setters of the app's `Tokens` global:
///
/// - `set_mode(ui: &$window, mode: Mode)`: light, dark or system. Takes effect at once; no restart.
/// - `is_dark(ui: &$window) -> bool`: whether the tokens currently resolve to the dark set.
/// - `set_reduced_motion(ui: &$window, on: bool)`: every `motion-*` duration becomes 0 while it is set.
/// - `set_text_scale(ui: &$window, scale: TextScale)`: multiplies every type size and line height.
///
/// `$window` is the root window type that owns the global, `$tokens` the generated `Tokens` global, and `$choice` the
/// generated `ThemeChoice` enum with variants `Light`, `Dark` and `System`.
#[macro_export]
macro_rules! wire_theme {
    ($window:path, $tokens:path, $choice:path $(,)?) => {
        /// Sets light, dark or system. Takes effect at once; no restart.
        pub fn set_mode(ui: &$window, mode: $crate::theme::Mode) {
            use ::slint::ComponentHandle as _;
            use $choice as __Choice;
            let choice = match mode {
                $crate::theme::Mode::Light => __Choice::Light,
                $crate::theme::Mode::Dark => __Choice::Dark,
                $crate::theme::Mode::System => __Choice::System,
            };
            ui.global::<$tokens>().set_theme(choice);
        }

        /// Whether the tokens currently resolve to the dark set.
        pub fn is_dark(ui: &$window) -> bool {
            use ::slint::ComponentHandle as _;
            ui.global::<$tokens>().get_dark()
        }

        /// The manual reduced-motion flag. Every `motion-*` duration becomes 0 while it is set.
        pub fn set_reduced_motion(ui: &$window, on: bool) {
            use ::slint::ComponentHandle as _;
            ui.global::<$tokens>().set_reduced_motion(on);
        }

        /// The text scale (100, 125, 150 or 200 per cent). Multiplies every type size and line height.
        pub fn set_text_scale(ui: &$window, scale: $crate::theme::TextScale) {
            use ::slint::ComponentHandle as _;
            ui.global::<$tokens>()
                .set_text_scale_percent(scale.percent() as i32);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{Mode, TextScale};

    #[test]
    fn percent_round_trips_and_factors_are_the_four_steps() {
        for s in TextScale::ALL {
            assert_eq!(TextScale::from_percent(s.percent()), Some(s));
        }
        assert_eq!(TextScale::from_percent(110), None);
        assert_eq!(TextScale::default(), TextScale::P100);
        let f: Vec<f32> = TextScale::ALL.iter().map(|s| s.factor()).collect();
        assert_eq!(f, vec![1.0, 1.25, 1.5, 2.0]);
    }

    #[test]
    fn the_default_mode_follows_the_system() {
        assert_eq!(Mode::default(), Mode::System);
    }
}
