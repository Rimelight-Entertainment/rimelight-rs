//! Plain-Rust page state of the ST5.4 page templates: what a page shows in place of its data (loading, empty, error, or
//! ready, where the page draws its own content). It has no Slint types. The app expands [`wire_page_state!`] in its own
//! crate to map a [`PageState`] to the `PageLoadState` enum that `rimelight_ui`'s `PageState` component takes.

/// What a page shows. `Ready` means the page draws its data; the other three replace it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageState {
    /// A load is running. Shown ahead of everything else.
    Loading,
    /// The data is on screen.
    Ready,
    /// The load worked and there is nothing to show.
    Empty,
    /// The load failed. The text is the message to show under the failure (empty when there is none).
    Error(String),
}

impl PageState {
    /// The state after a load. A running load is `Loading` whatever it has so far. A failure is an error, even when rows
    /// are left from an earlier load, so the page never shows stale rows as if they were new. Otherwise zero rows is
    /// `Empty` and any rows are `Ready`.
    pub fn after_load(loading: bool, error: Option<&str>, item_count: usize) -> PageState {
        if loading {
            PageState::Loading
        } else if let Some(message) = error {
            PageState::Error(message.to_string())
        } else if item_count == 0 {
            PageState::Empty
        } else {
            PageState::Ready
        }
    }

    /// True when the page draws its data.
    pub fn is_ready(&self) -> bool {
        matches!(self, PageState::Ready)
    }

    /// The error message, or an empty string for any other state.
    pub fn error_text(&self) -> &str {
        match self {
            PageState::Error(message) => message.as_str(),
            _ => "",
        }
    }
}

/// Emits, in the calling module, `load_state`: the `PageLoadState` of a [`PageState`]. `$load` is the app's generated
/// `PageLoadState` enum (variants `loading`, `loaded`, `empty` and `error`, which Slint names `Loading`, `Loaded`,
/// `Empty` and `Error` in Rust). Expand it once in the app crate that shows the page.
#[macro_export]
macro_rules! wire_page_state {
    ($load:path $(,)?) => {
        /// The `PageLoadState` a page state shows as.
        pub fn load_state(state: &$crate::page_state::PageState) -> $load {
            match state {
                $crate::page_state::PageState::Loading => <$load>::Loading,
                $crate::page_state::PageState::Ready => <$load>::Loaded,
                $crate::page_state::PageState::Empty => <$load>::Empty,
                $crate::page_state::PageState::Error(_) => <$load>::Error,
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_load_wins_over_rows_and_errors() {
        assert_eq!(
            PageState::after_load(true, Some("old failure"), 5),
            PageState::Loading
        );
    }

    #[test]
    fn a_failure_is_shown_even_with_rows_from_an_earlier_load() {
        let state = PageState::after_load(false, Some("Network down"), 12);
        assert_eq!(state, PageState::Error("Network down".to_string()));
        assert_eq!(state.error_text(), "Network down");
        assert!(!state.is_ready());
    }

    #[test]
    fn zero_rows_is_empty_and_rows_are_ready() {
        assert_eq!(PageState::after_load(false, None, 0), PageState::Empty);
        let ready = PageState::after_load(false, None, 3);
        assert_eq!(ready, PageState::Ready);
        assert!(ready.is_ready());
        assert_eq!(ready.error_text(), "");
    }

    #[test]
    fn an_empty_error_message_is_kept_as_empty_text() {
        let state = PageState::after_load(false, Some(""), 0);
        assert_eq!(state, PageState::Error(String::new()));
        assert_eq!(state.error_text(), "");
    }
}
