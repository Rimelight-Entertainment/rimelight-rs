//! Spike: can a live web view sit inside a Slint 1.18 window, over a rectangle
//! that Slint lays out, and stay aligned on resize and scroll?
//! Windows is the only platform with a real implementation (wry child window).
//! wasm32 (iframe) and android (JNI) are code-only sketches.

slint::include_modules!();

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "android")]
mod android;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.show()?;
    println!("window shown");

    #[cfg(windows)]
    let webview: Rc<RefCell<Option<wry::WebView>>> = Rc::new(RefCell::new(None));
    #[cfg(windows)]
    {
        let wv = webview.clone();
        ui.on_load(move |url| {
            if let Some(w) = wv.borrow().as_ref() {
                let _ = w.load_url(url.as_str());
                println!("load {url}");
            }
        });
    }
    #[cfg(not(windows))]
    println!("no native web view on this platform (code-only: wasm32, android)");

    // Poll the host rectangle; create the web view lazily once layout gives it a size.
    let weak = ui.as_weak();
    let last = Cell::new([0f32; 4]);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(16), move || {
        let Some(ui) = weak.upgrade() else { return };
        let r = [ui.get_host_x(), ui.get_host_y(), ui.get_host_width(), ui.get_host_height()];
        if r[2] <= 0.0 || r[3] <= 0.0 || r == last.get() {
            return;
        }
        last.set(r);
        #[cfg(windows)]
        place(&ui, &webview, r);
        #[cfg(target_arch = "wasm32")]
        web::place(r);
        #[cfg(not(any(windows, target_arch = "wasm32")))]
        let _ = &ui;
    });

    ui.run()?;
    Ok(())
}

#[cfg(windows)]
fn place(ui: &AppWindow, slot: &RefCell<Option<wry::WebView>>, r: [f32; 4]) {
    let [x, y, w, h] = r;
    let rect = wry::Rect {
        position: wry::dpi::LogicalPosition::new(x, y).into(),
        size: wry::dpi::LogicalSize::new(w, h).into(),
    };
    println!("bounds x={x} y={y} w={w} h={h}");

    let mut slot = slot.borrow_mut();
    if let Some(wv) = slot.as_ref() {
        let _ = wv.set_bounds(rect);
        return;
    }
    let handle = ui.window().window_handle();
    println!("handle ok");
    match wry::WebViewBuilder::new()
        .with_url("https://example.com")
        .with_bounds(rect)
        .build_as_child(&handle)
    {
        Ok(wv) => {
            println!("webview created");
            *slot = Some(wv);
        }
        Err(e) => println!("webview failed: {e}"),
    }
}
