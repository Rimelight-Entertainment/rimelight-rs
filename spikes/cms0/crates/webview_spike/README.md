# webview_spike

Question: can a live web page be shown inside a Slint 1.18.1 window, positioned over a rectangle Slint lays out, and kept in place on resize and scroll?

## Run

From the `spikes/cms0` folder:

    cargo run --bin webview_spike

## What to look for (Windows)

- A page (example.com) appears inside the grey-bordered box.
- Resize the window: the page stays inside the box.
- Scroll the list: the page stays aligned with the box.
- Click into the page and type: focus and input work.
- Known limit: Slint popups (menus, tooltips) opened over the box are hidden behind the web view, because the web view is a native child window.
- Stdout prints each step ("window shown", "handle ok", "webview created", bounds).

## Platforms

- Windows: working path, using wry `build_as_child`. Not yet built or verified.
- wasm32: code only (`src/web.rs`, an absolutely positioned iframe).
- Android: code only (`src/android.rs`, a JNI sketch with `todo!()` parts).
