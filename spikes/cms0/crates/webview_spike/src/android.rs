//! Code-only sketch for Android. NOT compiled or tested. Parts are todo!().
//!
//! Planned path: wry's Android support needs tao's own activity glue, which this
//! Slint app does not use. Instead, use JNI (the `jni` crate, not yet in Cargo.toml)
//! on the android-activity backend Slint already runs on:
//!   1. Get the JavaVM and Activity from the `AndroidApp` handle.
//!   2. Create an `android.webkit.WebView` and call `loadUrl` on it.
//!   3. Add it to the activity's content view with `FrameLayout.LayoutParams`
//!      built from the host-x/host-y/host-width/host-height properties.
//!   4. Re-layout on every timer tick, the same way the Windows path does.

#![allow(dead_code)]

pub fn place_webview(_x: f32, _y: f32, _w: f32, _h: f32) {
    todo!("JNI: create android.webkit.WebView, add to content view, set layout params");
}
