//! Code-only sketch for wasm32 (not compiled yet).
//! Slint renders into a <canvas>; this places an <iframe> absolutely over the
//! same rectangle as the Slint host box. Assumes the canvas sits at the body origin.

use std::cell::RefCell;

use wasm_bindgen::JsCast;
use web_sys::HtmlIFrameElement;

thread_local! {
    static FRAME: RefCell<Option<HtmlIFrameElement>> = const { RefCell::new(None) };
}

pub fn place([x, y, w, h]: [f32; 4]) {
    FRAME.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
            let Some(body) = doc.body() else { return };
            let Ok(el) = doc.create_element("iframe") else { return };
            let Ok(frame) = el.dyn_into::<HtmlIFrameElement>() else { return };
            frame.set_src("https://example.com");
            let _ = body.append_child(&frame);
            *slot = Some(frame);
        }
        if let Some(frame) = slot.as_ref() {
            let style = frame.style();
            let _ = style.set_property("position", "absolute");
            let _ = style.set_property("left", &format!("{x}px"));
            let _ = style.set_property("top", &format!("{y}px"));
            let _ = style.set_property("width", &format!("{w}px"));
            let _ = style.set_property("height", &format!("{h}px"));
        }
    });
}
