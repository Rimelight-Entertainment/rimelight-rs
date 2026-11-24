//! Native web views shown over a rectangle of a Slint window, for web-only content blocks (decision 0049).
//!
//! The app owns one [`WebViews`] on the UI thread and calls, for each web block:
//!
//! - [`WebViews::show`] every frame or timer tick with the block's current geometry. Each key is one native view. The
//!   view is created on the first call, and it moves or resizes only when the rectangle, the scale or the URL changed.
//! - [`WebViews::hide_all`] while a Slint menu or dialog is open. A native child view always draws on top of Slint, so
//!   the popup would otherwise sit behind the web view.
//! - [`WebViews::close`] when the block leaves the screen. The native view is then destroyed.
//!
//! Rectangles are logical pixels of the Slint window (x and y from the window's top-left). `scale` is the window's
//! scale factor. It turns logical pixels into physical ones on Android. Windows and the web take logical pixels.
//!
//! Per target:
//!
//! - Windows (`windows.rs`): a wry child web view on the window handle (`build_as_child`).
//! - Android (`android::webview`): an `android.webkit.WebView` in the activity's content view, through JNI. Its calls
//!   run on the main thread only, and `show` and the other calls return an error elsewhere.
//! - wasm32 (`web.rs`): an absolutely positioned `<iframe>` over the canvas. It assumes the canvas is at the page origin.
//! - macOS, iOS, Linux: `show` returns [`WebViewError::Unsupported`]. `hide` and `close` do nothing.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use self::windows as imp;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
use self::web as imp;

#[cfg(target_os = "android")]
use crate::android::webview as imp;

#[cfg(not(any(windows, target_os = "android", target_arch = "wasm32")))]
mod imp {
    use raw_window_handle::HasWindowHandle;

    use super::{Placement, WebViewError};

    #[derive(Default)]
    pub struct Views;

    impl Views {
        pub fn new() -> Self {
            Self
        }
        pub fn show(
            &mut self,
            _window: &impl HasWindowHandle,
            _key: &str,
            _placement: Placement,
            _url: &str,
        ) -> Result<(), WebViewError> {
            Err(WebViewError::Unsupported)
        }
        pub fn hide(&mut self, _key: &str) {}
        pub fn hide_all(&mut self) {}
        pub fn close(&mut self, _key: &str) {}
    }
}

use raw_window_handle::HasWindowHandle;

/// Why a web view could not be shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebViewError {
    /// This target has no native web view (macOS, iOS, Linux).
    Unsupported,
    /// The platform refused or failed. The text says what, for the log.
    Platform(String),
}

impl std::fmt::Display for WebViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebViewError::Unsupported => write!(f, "web views are not supported on this platform"),
            WebViewError::Platform(text) => write!(f, "web view: {text}"),
        }
    }
}

impl std::error::Error for WebViewError {}

/// The rectangle, scale and the platform's view of them. Two equal placements need no move.
#[cfg_attr(
    not(any(windows, target_os = "android", target_arch = "wasm32")),
    allow(dead_code)
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub scale: f32,
}

/// True when `now` differs from the placement last applied to the view (or none was applied yet).
#[cfg_attr(
    not(any(windows, target_os = "android", target_arch = "wasm32")),
    allow(dead_code)
)]
pub(crate) fn placement_changed(last: Option<Placement>, now: Placement) -> bool {
    last != Some(now)
}

/// A logical length in physical pixels: `logical * scale`, rounded to the nearest pixel.
/// Only the Android view sizes itself in physical pixels.
#[cfg(any(target_os = "android", test))]
pub(crate) fn to_physical(logical: f32, scale: f32) -> i32 {
    (logical * scale).round() as i32
}

/// The native web views of one window, one per key. Create it on the UI thread and keep it for the life of the window.
pub struct WebViews {
    inner: imp::Views,
}

impl WebViews {
    pub fn new() -> Self {
        Self {
            inner: imp::Views::new(),
        }
    }

    /// Shows (creating if needed) the view `key` at the logical rectangle (`x`, `y`, `w`, `h`), loading `url` if it
    /// changed. Call it every frame or timer tick: the view moves only when the rectangle changed. `window` is the Slint
    /// window's handle (Windows needs it; the other targets ignore it).
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        window: &impl HasWindowHandle,
        key: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        scale: f32,
        url: &str,
    ) -> Result<(), WebViewError> {
        self.inner
            .show(window, key, Placement { x, y, w, h, scale }, url)
    }

    /// Hides the view `key` (it keeps its page). Call it while Slint menus or dialogs are open.
    pub fn hide(&mut self, key: &str) {
        self.inner.hide(key);
    }

    /// Hides every view. Call it while Slint menus or dialogs are open, and show the views again on the next `show`.
    pub fn hide_all(&mut self) {
        self.inner.hide_all();
    }

    /// Destroys the view `key`. Call it when the block leaves.
    pub fn close(&mut self, key: &str) {
        self.inner.close(key);
    }
}

impl Default for WebViews {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Placement, placement_changed, to_physical};

    fn at(x: f32, y: f32, w: f32, h: f32, scale: f32) -> Placement {
        Placement { x, y, w, h, scale }
    }

    #[test]
    fn first_placement_always_applies() {
        assert!(placement_changed(None, at(10.0, 20.0, 300.0, 200.0, 1.0)));
    }

    #[test]
    fn unchanged_rectangle_is_not_moved() {
        let placed = at(10.0, 20.0, 300.0, 200.0, 1.5);
        assert!(!placement_changed(Some(placed), placed));
    }

    #[test]
    fn any_change_of_position_size_or_scale_is_applied() {
        let placed = at(10.0, 20.0, 300.0, 200.0, 1.0);
        assert!(placement_changed(
            Some(placed),
            at(11.0, 20.0, 300.0, 200.0, 1.0)
        ));
        assert!(placement_changed(
            Some(placed),
            at(10.0, 21.0, 300.0, 200.0, 1.0)
        ));
        assert!(placement_changed(
            Some(placed),
            at(10.0, 20.0, 301.0, 200.0, 1.0)
        ));
        assert!(placement_changed(
            Some(placed),
            at(10.0, 20.0, 300.0, 201.0, 1.0)
        ));
        assert!(placement_changed(
            Some(placed),
            at(10.0, 20.0, 300.0, 200.0, 2.0)
        ));
    }

    #[test]
    fn logical_to_physical_scales_and_rounds() {
        assert_eq!(to_physical(100.0, 1.5), 150);
        assert_eq!(to_physical(100.0, 1.0), 100);
        assert_eq!(to_physical(33.3, 1.5), 50);
        assert_eq!(to_physical(0.0, 2.75), 0);
    }

    #[test]
    fn logical_to_physical_keeps_negative_offsets() {
        assert_eq!(to_physical(-20.0, 1.5), -30);
    }
}
