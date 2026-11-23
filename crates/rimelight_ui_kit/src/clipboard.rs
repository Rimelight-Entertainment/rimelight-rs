//! Copy text to the clipboard from a Slint app (the shared form of rimelight-space's `clipboard.rs`).
//!
//! Slint has no clipboard call of its own. The app keeps a hidden writer in its markup (a component with a TextInput,
//! whose `copy()` Slint hands to the platform clipboard) and exposes a window function that sets its text and copies
//! it. [`wire_clipboard!`] emits the Rust side that calls that function. The app's markup must declare:
//!
//! ```text
//! export component AppWindow inherits Window {
//!     public function copy-text(text: string) {
//!         clipboard-writer.copy(text);   // sets the hidden TextInput's text, selects it, calls copy()
//!     }
//!     clipboard-writer := ClipboardWriter {}
//! }
//! ```
//!
//! On the desktop the write is synchronous. Web apps need a browser writer (`navigator.clipboard.writeText`), which
//! answers later; register it with the app's own code and use [`Outcome`] for the result.
//!
//! This crate names no generated type. The macro is expanded in the app crate, where the window type is in scope.

/// The result of a copy, for the toast that follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The text was written to the clipboard.
    Copied,
    /// The clipboard refused the text.
    Refused,
}

/// The toast text and tone for a copy outcome: "Copied" in the success tone, or the refusal in the danger tone.
/// `label` names what was copied, for example "Link" or "Header value".
pub fn toast_for(outcome: Outcome, label: &str) -> (crate::toast::Tone, String) {
    match outcome {
        Outcome::Copied => (crate::toast::Tone::Success, format!("{label} copied")),
        Outcome::Refused => (
            crate::toast::Tone::Danger,
            format!("Could not copy the {}", label.to_lowercase()),
        ),
    }
}

/// Emits, in the calling module, `copy_text(ui: &$window, text: &str) -> bool`: calls the window's `copy-text` function
/// (see the module docs) and reports the write as done. On desktop the write is synchronous, so the outcome is known at
/// once.
#[macro_export]
macro_rules! wire_clipboard {
    ($window:path $(,)?) => {
        /// Copies `text` to the clipboard through the window's hidden writer. Returns `true` when it was handed over.
        pub fn copy_text(ui: &$window, text: &str) -> bool {
            use ::slint::ComponentHandle as _;
            ui.invoke_copy_text(text.into());
            true
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{Outcome, toast_for};
    use crate::toast::Tone;

    #[test]
    fn copy_outcome_picks_the_success_or_error_toast() {
        assert_eq!(
            toast_for(Outcome::Copied, "Link"),
            (Tone::Success, "Link copied".to_owned())
        );
        assert_eq!(
            toast_for(Outcome::Refused, "Link"),
            (Tone::Danger, "Could not copy the link".to_owned())
        );
    }
}
