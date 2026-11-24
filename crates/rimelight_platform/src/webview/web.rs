//! wasm32 web views: an absolutely positioned `<iframe>` per key, over the Slint canvas.
//!
//! Slint draws into a `<canvas>`, whose CSS pixels are the logical pixels used here. The frame is placed in the page
//! body, so it assumes the canvas sits at the page origin (0, 0). The scale factor is not used on the web. A frame is
//! removed from the page by `close`.

use std::collections::HashMap;

use raw_window_handle::HasWindowHandle;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlIFrameElement;

use super::{Placement, WebViewError, placement_changed};

/// One iframe per key.
#[derive(Default)]
pub struct Views {
    frames: HashMap<String, Frame>,
}

struct Frame {
    element: HtmlIFrameElement,
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
        _window: &impl HasWindowHandle,
        key: &str,
        placement: Placement,
        url: &str,
    ) -> Result<(), WebViewError> {
        if let Some(frame) = self.frames.get_mut(key) {
            if placement_changed(frame.placement, placement) {
                place(&frame.element, placement);
                frame.placement = Some(placement);
            }
            if frame.url != url {
                frame.element.set_src(url);
                frame.url = url.to_owned();
            }
            if !frame.visible {
                set_display(&frame.element, "block");
                frame.visible = true;
            }
            return Ok(());
        }

        let element = create_frame()?;
        element.set_src(url);
        place(&element, placement);
        self.frames.insert(
            key.to_owned(),
            Frame {
                element,
                url: url.to_owned(),
                placement: Some(placement),
                visible: true,
            },
        );
        Ok(())
    }

    pub fn hide(&mut self, key: &str) {
        if let Some(frame) = self.frames.get_mut(key) {
            hide_frame(frame);
        }
    }

    pub fn hide_all(&mut self) {
        for frame in self.frames.values_mut() {
            hide_frame(frame);
        }
    }

    /// Removes the iframe from the page.
    pub fn close(&mut self, key: &str) {
        if let Some(frame) = self.frames.remove(key) {
            if let Some(parent) = frame.element.parent_node() {
                let _ = parent.remove_child(&frame.element);
            }
        }
    }
}

fn hide_frame(frame: &mut Frame) {
    if frame.visible {
        set_display(&frame.element, "none");
        frame.visible = false;
    }
}

/// Adds an iframe at the end of the page body.
fn create_frame() -> Result<HtmlIFrameElement, WebViewError> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| WebViewError::Platform("no document".to_owned()))?;
    let body = document
        .body()
        .ok_or_else(|| WebViewError::Platform("no document body".to_owned()))?;
    let element = document.create_element("iframe").map_err(js_error)?;
    // UNSURE: `dyn_into` hands back the `Element` on failure, not a `JsValue`, so the failure text is fixed here.
    let frame: HtmlIFrameElement = element
        .dyn_into()
        .map_err(|_| WebViewError::Platform("created element is not an iframe".to_owned()))?;
    body.append_child(&frame).map_err(js_error)?;
    Ok(frame)
}

fn place(frame: &HtmlIFrameElement, placement: Placement) {
    let style = frame.style();
    let _ = style.set_property("position", "absolute");
    let _ = style.set_property("left", &format!("{}px", placement.x));
    let _ = style.set_property("top", &format!("{}px", placement.y));
    let _ = style.set_property("width", &format!("{}px", placement.w));
    let _ = style.set_property("height", &format!("{}px", placement.h));
}

fn set_display(frame: &HtmlIFrameElement, display: &str) {
    let _ = frame.style().set_property("display", display);
}

fn js_error(value: JsValue) -> WebViewError {
    WebViewError::Platform(format!("{value:?}"))
}
