//! Native web views over web-only blocks. The app owns the web views (AGENTS.md: a web view is a native child window,
//! and only the app can create one). This module defines the seam the app implements and the URL the embed route
//! takes. The editor reports each web-only block's absolute box through `HostEvent::WebGeometry` (lib.rs), and the app
//! calls [`WebViewHost::show`] with it.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// What the app does with a native web view. `key` is the block id, so one block keeps one web view.
pub trait WebViewHost {
    /// Shows (or moves and reloads) the web view of `key` over the box `x, y, w, h` in logical pixels, loading `url`.
    fn show(&mut self, key: &str, x: f32, y: f32, w: f32, h: f32, url: &str);
    /// Hides the web view of `key`. Called when its block leaves the page.
    fn hide(&mut self, key: &str);
    /// Hides every web view. Called while a menu, popup or dialog is open (a native child window draws over Slint),
    /// and the editor then shows the block's snapshot image in their place.
    fn hide_all(&mut self);
}

/// The embed route for one web-only block: `{base}/embed/block#<base64url of the block JSON>`. The block JSON is in
/// the fragment, so it is not sent to a server in a request line. The base has no trailing slash.
pub fn embed_url(base: &str, block_json: &serde_json::Value) -> String {
    let json = serde_json::to_string(block_json).unwrap_or_else(|_| "null".to_string());
    format!(
        "{}/embed/block#{}",
        base.trim_end_matches('/'),
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_block_json_round_trips_through_the_fragment() {
        let block = json!({"id": "b1", "type": "LivePreviewBlock", "props": {"height": 320}});
        let url = embed_url("https://example.test/", &block);
        let (prefix, fragment) = url.split_once('#').expect("fragment");
        assert_eq!(prefix, "https://example.test/embed/block");
        let bytes = URL_SAFE_NO_PAD.decode(fragment).expect("base64url");
        let back: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(back, block);
    }
}
