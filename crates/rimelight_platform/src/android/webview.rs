//! Android web views: an `android.webkit.WebView` per key, added to the activity's content view (a `FrameLayout`) through
//! JNI, placed at physical pixels (logical * scale).
//!
//! Audit (unsafe blocks; each has a `// SAFETY:` comment at the block):
//! - `with_context`: `JavaVM::from_raw` on the VM pointer and `JObject::from_raw` on the activity pointer that ndk-context
//!   publishes for the process. Both stay valid while the app runs. The activity reference is wrapped in `ManuallyDrop`,
//!   so it is not deleted here.
//!
//! Threading: the view calls run only on the main looper thread, the Android UI thread. `on_ui_thread` checks that and
//! returns an error elsewhere. The app calls these from its Slint event loop, which runs on that thread. The
//! `Activity.runOnUiThread` hop is not used: it needs a Java `Runnable`, and this crate ships no Java class.
//!
//! Every JNI call that fails clears any pending Java exception (`check`), so the next call runs.
//!
//! No other unsafe code is here. Everything else is safe jni API calls.

use std::collections::HashMap;
use std::mem::ManuallyDrop;

use jni::objects::{GlobalRef, JObject, JValue};
use jni::{JNIEnv, JavaVM};
use raw_window_handle::HasWindowHandle;

use crate::webview::{Placement, WebViewError, placement_changed, to_physical};

/// `android.R.id.content`: the activity's content view, a `FrameLayout`.
const CONTENT_VIEW_ID: i32 = 0x0102_0002;
/// `View.VISIBLE`.
const VISIBLE: i32 = 0;
/// `View.GONE`.
const GONE: i32 = 8;

/// One `android.webkit.WebView` per key.
#[derive(Default)]
pub struct Views {
    views: HashMap<String, Entry>,
}

struct Entry {
    view: GlobalRef,
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
        let width = to_physical(placement.w, placement.scale).max(0);
        let height = to_physical(placement.h, placement.scale).max(0);
        let x = to_physical(placement.x, placement.scale);
        let y = to_physical(placement.y, placement.scale);
        let created = !self.views.contains_key(key);
        on_ui_thread(|env, activity| {
            if created {
                let view = create_view(env, activity, width, height)?;
                self.views.insert(
                    key.to_owned(),
                    Entry {
                        view,
                        url: String::new(),
                        placement: None,
                        visible: false,
                    },
                );
            }
            let Some(entry) = self.views.get_mut(key) else {
                return Ok(());
            };
            if placement_changed(entry.placement, placement) {
                layout(env, entry.view.as_obj(), width, height, x, y)?;
                entry.placement = Some(placement);
            }
            if entry.url != url {
                load_url(env, entry.view.as_obj(), url)?;
                entry.url = url.to_owned();
            }
            if !entry.visible {
                set_visibility(env, entry.view.as_obj(), VISIBLE)?;
                entry.visible = true;
            }
            Ok(())
        })
        .map_err(WebViewError::Platform)
    }

    pub fn hide(&mut self, key: &str) {
        let Some(entry) = self.views.get_mut(key) else {
            return;
        };
        if !entry.visible {
            return;
        }
        let hidden = on_ui_thread(|env, _| set_visibility(env, entry.view.as_obj(), GONE));
        if hidden.is_ok() {
            entry.visible = false;
        }
    }

    pub fn hide_all(&mut self) {
        let keys: Vec<String> = self.views.keys().cloned().collect();
        for key in keys {
            self.hide(&key);
        }
    }

    /// Removes the view from the content view and destroys it. The reference is dropped even if the UI-thread check fails,
    /// so the view leaves the bookkeeping either way.
    pub fn close(&mut self, key: &str) {
        let Some(entry) = self.views.remove(key) else {
            return;
        };
        let _ = on_ui_thread(|env, activity| {
            let content = content_view(env, activity)?;
            let result = env.call_method(
                &content,
                "removeView",
                "(Landroid/view/View;)V",
                &[JValue::Object(entry.view.as_obj())],
            );
            check(env, result)?;
            let result = env.call_method(entry.view.as_obj(), "destroy", "()V", &[]);
            check(env, result)?;
            Ok(())
        });
    }
}

/// Creates the web view, enables JavaScript and adds it to the content view. Its position and size are set by `layout`
/// on the first `show`.
fn create_view(
    env: &mut JNIEnv<'_>,
    activity: &JObject<'_>,
    width: i32,
    height: i32,
) -> Result<GlobalRef, String> {
    let result = env.new_object(
        "android/webkit/WebView",
        "(Landroid/content/Context;)V",
        &[JValue::Object(activity)],
    );
    let view = check(env, result)?;
    let result = env
        .call_method(&view, "getSettings", "()Landroid/webkit/WebSettings;", &[])
        .and_then(|value| value.l());
    let settings = check(env, result)?;
    let result = env.call_method(
        &settings,
        "setJavaScriptEnabled",
        "(Z)V",
        &[JValue::Bool(1)],
    );
    check(env, result)?;
    let content = content_view(env, activity)?;
    let params = layout_params(env, width, height)?;
    let result = env.call_method(
        &content,
        "addView",
        "(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V",
        &[JValue::Object(&view), JValue::Object(&params)],
    );
    check(env, result)?;
    let result = env.new_global_ref(&view);
    check(env, result)
}

/// Sets the view's size (a new `FrameLayout.LayoutParams`) and its position (`setX`, `setY`), in pixels.
fn layout(
    env: &mut JNIEnv<'_>,
    view: &JObject<'_>,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let params = layout_params(env, width, height)?;
    let result = env.call_method(
        view,
        "setLayoutParams",
        "(Landroid/view/ViewGroup$LayoutParams;)V",
        &[JValue::Object(&params)],
    );
    check(env, result)?;
    let result = env.call_method(view, "setX", "(F)V", &[JValue::Float(x as f32)]);
    check(env, result)?;
    let result = env.call_method(view, "setY", "(F)V", &[JValue::Float(y as f32)]);
    check(env, result)?;
    Ok(())
}

fn layout_params<'a>(env: &mut JNIEnv<'a>, width: i32, height: i32) -> Result<JObject<'a>, String> {
    let result = env.new_object(
        "android/widget/FrameLayout$LayoutParams",
        "(II)V",
        &[JValue::Int(width), JValue::Int(height)],
    );
    check(env, result)
}

fn load_url(env: &mut JNIEnv<'_>, view: &JObject<'_>, url: &str) -> Result<(), String> {
    let result = env.new_string(url);
    let text = check(env, result)?;
    let result = env.call_method(
        view,
        "loadUrl",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&text)],
    );
    check(env, result)?;
    Ok(())
}

/// `View.setVisibility` with `VISIBLE` or `GONE`.
fn set_visibility(env: &mut JNIEnv<'_>, view: &JObject<'_>, visibility: i32) -> Result<(), String> {
    let result = env.call_method(view, "setVisibility", "(I)V", &[JValue::Int(visibility)]);
    check(env, result)?;
    Ok(())
}

/// The activity's content view (`findViewById(android.R.id.content)`), a `FrameLayout`.
fn content_view<'a>(env: &mut JNIEnv<'a>, activity: &JObject<'_>) -> Result<JObject<'a>, String> {
    let result = env
        .call_method(
            activity,
            "findViewById",
            "(I)Landroid/view/View;",
            &[JValue::Int(CONTENT_VIEW_ID)],
        )
        .and_then(|value| value.l());
    let content = check(env, result)?;
    if content.is_null() {
        return Err("web view: the activity has no content view".to_owned());
    }
    Ok(content)
}

/// Turns a JNI failure into a message. A Java exception left pending is cleared, so the next JNI call runs.
fn check<T>(env: &mut JNIEnv<'_>, result: jni::errors::Result<T>) -> Result<T, String> {
    result.map_err(|error| {
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            "web view: a Java call threw an exception".to_owned()
        } else {
            format!("web view: {error}")
        }
    })
}

/// Runs `f` with a JNI environment and the activity, on the main looper thread only. Any other thread gets an error,
/// because the view calls must run on the UI thread.
fn on_ui_thread<T>(
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> Result<T, String>,
) -> Result<T, String> {
    with_context(|env, activity| {
        let result = env
            .call_static_method(
                "android/os/Looper",
                "myLooper",
                "()Landroid/os/Looper;",
                &[],
            )
            .and_then(|value| value.l());
        let current = check(env, result)?;
        let result = env
            .call_static_method(
                "android/os/Looper",
                "getMainLooper",
                "()Landroid/os/Looper;",
                &[],
            )
            .and_then(|value| value.l());
        let main = check(env, result)?;
        let result = env.is_same_object(&current, &main);
        if !check(env, result)? {
            return Err("web view: the calls must run on the UI thread".to_owned());
        }
        f(env, activity)
    })
}

/// Runs `f` on this thread with a JNI environment and the activity object. Both come from ndk-context and stay valid for
/// the life of the app.
fn with_context<T>(
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let context = ndk_context::android_context();
    // SAFETY: ndk-context holds the process's JavaVM from when android-activity starts the app until it exits, and this
    // runs on a thread of that process.
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }
        .map_err(|error| format!("web view: {error}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|error| format!("web view: {error}"))?;
    // SAFETY: the context pointer is the activity's global reference, kept by ndk-context for the life of the app. It is
    // wrapped in ManuallyDrop, so the reference is not deleted here.
    let activity = ManuallyDrop::new(unsafe { JObject::from_raw(context.context().cast()) });
    f(&mut env, &activity)
}
