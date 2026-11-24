//! Android calls through JNI: open a URL in the system browser, read the data of the activity's intent (a sign-in return),
//! and save a download to the app's Downloads folder.
//!
//! Audit (unsafe blocks; each has a `// SAFETY:` comment at the block):
//! - `with_activity`: `JavaVM::from_raw` on the VM pointer android-activity owns, on the thread it attached.
//! - `with_activity`: `JObject::from_raw` on the NativeActivity pointer, wrapped in `ManuallyDrop` so the global
//!   reference is never deleted here.
//! - `webview::with_context` (see `webview`): the same two pointers, read from ndk-context instead of `AndroidApp`.
//!
//! Everything else is safe jni API calls.

use std::fmt::Display;
use std::mem::ManuallyDrop;

use jni::objects::{JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use slint::android::AndroidApp;

/// Native web views (`android.webkit.WebView`) over the activity's content view, for `crate::webview`.
pub(crate) mod webview;

/// Opens `url` with ACTION_VIEW from the activity, so the system browser (or the app that handles the scheme) takes it.
pub fn open_url(app: &AndroidApp, url: &str) -> Result<(), String> {
    with_activity(app, |env, activity| {
        let action = env
            .get_static_field(
                "android/content/Intent",
                "ACTION_VIEW",
                "Ljava/lang/String;",
            )
            .and_then(|value| value.l())
            .map_err(err)?;
        let text = env.new_string(url).map_err(err)?;
        let uri = env
            .call_static_method(
                "android/net/Uri",
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&text)],
            )
            .and_then(|value| value.l())
            .map_err(err)?;
        let intent = env
            .new_object(
                "android/content/Intent",
                "(Ljava/lang/String;Landroid/net/Uri;)V",
                &[JValue::Object(&action), JValue::Object(&uri)],
            )
            .map_err(err)?;
        env.call_method(
            activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)],
        )
        .map_err(err)?;
        Ok(())
    })
}

/// The data URI of the activity's current intent, if it has one. The app keeps the newest intent (see
/// `RimelightActivity` in the Android project), so a sign-in return is read here after a new intent.
pub fn intent_data(app: &AndroidApp) -> Result<Option<String>, String> {
    with_activity(app, |env, activity| {
        let intent = env
            .call_method(activity, "getIntent", "()Landroid/content/Intent;", &[])
            .and_then(|value| value.l())
            .map_err(err)?;
        if intent.is_null() {
            return Ok(None);
        }
        let data = env
            .call_method(&intent, "getDataString", "()Ljava/lang/String;", &[])
            .and_then(|value| value.l())
            .map_err(err)?;
        if data.is_null() {
            return Ok(None);
        }
        let data = JString::from(data);
        let text = String::from(env.get_string(&data).map_err(err)?);
        Ok(Some(text))
    })
}

/// Writes `bytes` to a file called `name` in the app's Downloads folder (its external files folder for Downloads, which
/// needs no storage permission on API 26 and later). Returns the path written. An existing file of that name is replaced.
pub fn save_download(app: &AndroidApp, name: &str, bytes: &[u8]) -> Result<String, String> {
    with_activity(app, |env, activity| {
        let directory_name = env
            .get_static_field(
                "android/os/Environment",
                "DIRECTORY_DOWNLOADS",
                "Ljava/lang/String;",
            )
            .and_then(|value| value.l())
            .map_err(err)?;
        let directory = env
            .call_method(
                activity,
                "getExternalFilesDir",
                "(Ljava/lang/String;)Ljava/io/File;",
                &[JValue::Object(&directory_name)],
            )
            .and_then(|value| value.l())
            .map_err(err)?;
        if directory.is_null() {
            return Err("the Downloads folder is not available".to_owned());
        }
        let child = env.new_string(name).map_err(err)?;
        let file = env
            .new_object(
                "java/io/File",
                "(Ljava/io/File;Ljava/lang/String;)V",
                &[JValue::Object(&directory), JValue::Object(&child)],
            )
            .map_err(err)?;
        let stream = env
            .new_object(
                "java/io/FileOutputStream",
                "(Ljava/io/File;)V",
                &[JValue::Object(&file)],
            )
            .map_err(err)?;
        let data = env.byte_array_from_slice(bytes).map_err(err)?;
        let written = env.call_method(&stream, "write", "([B)V", &[JValue::Object(&data)]);
        settle(env, written)?;
        let closed = env.call_method(&stream, "close", "()V", &[]);
        settle(env, closed)?;
        let path = env
            .call_method(&file, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .and_then(|value| value.l())
            .map_err(err)?;
        let path = JString::from(path);
        Ok(String::from(env.get_string(&path).map_err(err)?))
    })
}

/// Turns a JNI call's failure into its message. A Java exception left pending is cleared, so the next JNI call runs.
fn settle<T, E: Display>(env: &mut JNIEnv<'_>, result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| {
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
            "a file could not be written".to_owned()
        } else {
            err(error)
        }
    })
}

/// Runs `f` on this thread with a JNI environment and the NativeActivity object. Both come from android-activity and stay
/// valid for the life of the activity.
fn with_activity<T>(
    app: &AndroidApp,
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> Result<T, String>,
) -> Result<T, String> {
    // SAFETY: android-activity owns the JavaVM for the process, and this runs on the thread android-activity attached.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(err)?;
    let mut env = vm.attach_current_thread().map_err(err)?;
    // SAFETY: the activity pointer is android-activity's global reference to the NativeActivity. It is wrapped in
    // ManuallyDrop, so the reference is not deleted here.
    let activity = ManuallyDrop::new(unsafe { JObject::from_raw(app.activity_as_ptr().cast()) });
    f(&mut env, &activity)
}

fn err<E: Display>(error: E) -> String {
    error.to_string()
}
