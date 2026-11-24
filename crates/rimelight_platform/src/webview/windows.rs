//! Windows web views: a wry child web view on the Slint window (`build_as_child`), placed at logical pixels.
//!
//! wry creates the WebView2 control as a child window of the Slint window, so every call here runs on the UI thread that
//! owns the window. Native child windows draw on top of Slint, which is why the app calls `hide_all` while menus are open.
//! No unsafe code is needed: wry and raw-window-handle hold the window handle.

use std::collections::HashMap;

use raw_window_handle::HasWindowHandle;
use wry::dpi::{LogicalPosition, LogicalSize};

use super::{Placement, WebViewError, placement_changed};

/// One wry child view per key.
#[derive(Default)]
pub struct Views {
    views: HashMap<String, Entry>,
}

struct Entry {
    view: wry::WebView,
    url: String,
    placement: Option<Placement>,
    visible: bool,
}

impl Views {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(
        &mut self,
        window: &impl HasWindowHandle,
        key: &str,
        placement: Placement,
        url: &str,
    ) -> Result<(), WebViewError> {
        if let Some(entry) = self.views.get_mut(key) {
            if placement_changed(entry.placement, placement) {
                entry.view.set_bounds(bounds(placement)).map_err(platform)?;
                entry.placement = Some(placement);
            }
            if entry.url != url {
                entry.view.load_url(url).map_err(platform)?;
                entry.url = url.to_owned();
            }
            if !entry.visible {
                entry.view.set_visible(true).map_err(platform)?;
                entry.visible = true;
            }
            return Ok(());
        }

        let view = wry::WebViewBuilder::new()
            .with_url(url)
            .with_bounds(bounds(placement))
            .build_as_child(window)
            .map_err(platform)?;
        self.views.insert(
            key.to_owned(),
            Entry {
                view,
                url: url.to_owned(),
                placement: Some(placement),
                visible: true,
            },
        );
        Ok(())
    }

    pub fn hide(&mut self, key: &str) {
        if let Some(entry) = self.views.get_mut(key) {
            set_visible(entry, false);
        }
    }

    pub fn hide_all(&mut self) {
        for entry in self.views.values_mut() {
            set_visible(entry, false);
        }
    }

    /// Drops the wry view, which destroys its child window.
    pub fn close(&mut self, key: &str) {
        self.views.remove(key);
    }
}

fn set_visible(entry: &mut Entry, visible: bool) {
    if entry.visible != visible && entry.view.set_visible(visible).is_ok() {
        entry.visible = visible;
    }
}

/// The logical rectangle wry takes. Position and size are logical, so the window's scale factor is not needed here.
fn bounds(placement: Placement) -> wry::Rect {
    wry::Rect {
        position: LogicalPosition::new(placement.x, placement.y).into(),
        size: LogicalSize::new(placement.w, placement.h).into(),
    }
}

fn platform(error: impl std::fmt::Display) -> WebViewError {
    WebViewError::Platform(error.to_string())
}
