//! rimelight_platform: the OS calls Rimelight Media and Rimelight Space share, behind one portable API.
//!
//! - `win32` (Windows only): screen capture with GDI, top-level window listing and window styling (alpha, z-order,
//!   owner), the Win+Shift+S hook, the start lock, Recycle Bin, known folders, print, Share and text recognition.
//! - `android` (Android only): open a URL, read the activity's intent data and save a download, through JNI.
//!
//! Every function at the crate root works on every target. Where the OS has no such call the answer is empty
//! (`None`, `false`, an empty list) or an `Err` naming the platform, so callers need no `cfg` of their own.
//!
//! Unsafe code is denied crate-wide. It is allowed only in `win32` and `android`, and each unsafe block there has a
//! `// SAFETY:` comment (the audit notes at the top of those modules list them).

#[cfg(windows)]
#[allow(unsafe_code)]
pub mod win32;

#[cfg(target_os = "android")]
#[allow(unsafe_code)]
pub mod android;

#[cfg(windows)]
use win32 as imp;

#[cfg(not(windows))]
use portable as imp;

/// A rectangle in physical screen pixels (virtual-desktop coordinates, can be negative).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScreenRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// A top-level window: its handle (an `HWND` as `isize`), its title and its bounds on screen. Bounds are the default
/// rectangle when the listing did not need them (see [`WindowList::popout_owners`]).
#[derive(Debug, Clone)]
pub struct TopWindow {
    pub hwnd: isize,
    pub title: String,
    pub rect: ScreenRect,
}

/// Which top-level windows [`list_windows`] returns. Two presets cover the two apps; both rules are kept:
///
/// - [`WindowList::capturable`] (Media, snips and recordings): skips hidden, minimised, tool and cloaked windows and any
///   window of this process except `keep` (the main window); a window without bounds is skipped. No limit.
/// - [`WindowList::popout_owners`] (Space, the owner picker of a pop-out): visible titled windows of any process, except
///   `exclude`; a window without bounds is listed with a default rectangle; at most 200 windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowList {
    /// Windows of this process that are still listed (capturable mode only).
    pub keep: Option<isize>,
    /// A window never listed (the caller's own window).
    pub exclude: Option<isize>,
    /// True: the capture rules (see the type docs). False: the pop-out owner rules.
    pub capturable_only: bool,
    /// The most windows to return; None for no limit.
    pub limit: Option<usize>,
}

impl WindowList {
    /// The capture rules, keeping `keep` (the main window) in the list.
    pub fn capturable(keep: isize) -> Self {
        Self {
            keep: Some(keep),
            exclude: None,
            capturable_only: true,
            limit: None,
        }
    }

    /// The pop-out owner rules, leaving out `exclude` (the pop-out itself).
    pub fn popout_owners(exclude: isize) -> Self {
        Self {
            keep: None,
            exclude: Some(exclude),
            capturable_only: false,
            limit: Some(200),
        }
    }
}

/// The user folders Windows keeps for the apps (OneDrive can move them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserFolder {
    Pictures,
    Videos,
    Music,
}

/// Copies screen pixels into one reusable bitmap (BGRA rows, top-down). On a target without screen capture,
/// [`ScreenGrabber::new`] returns `None`.
pub use imp::ScreenGrabber;

/// The bounding box of every monitor, in physical pixels.
pub fn virtual_screen() -> ScreenRect {
    imp::virtual_screen()
}

/// The pointer position and the rectangle of the monitor it is on (physical pixels).
pub fn cursor_monitor() -> Option<((i32, i32), ScreenRect)> {
    imp::cursor_monitor()
}

/// Visible top-level windows matching `list`, front-most first.
pub fn list_windows(list: WindowList) -> Vec<TopWindow> {
    imp::list_windows(list)
}

/// Tells Explorer that file associations changed, so icons and "Open with" refresh.
pub fn file_types_changed() {
    imp::file_types_changed()
}

/// Keeps one of our windows out of screenshots and recordings (WDA_EXCLUDEFROMCAPTURE). `hwnd` is an `HWND` as `isize`.
pub fn exclude_from_capture(hwnd: isize) {
    imp::exclude_from_capture(hwnd)
}

/// Turns off the open and close animations for every window of `window_class` from its creation on. Call once on the UI
/// thread, before any window is shown. The class is the one set on the window (winit's `with_class_name`).
pub fn start_no_window_animations(window_class: &str) {
    imp::start_no_window_animations(window_class)
}

/// Starts the Win+Shift+S hook (once). Win+Shift+S is then swallowed and reported by [`take_win_shift_s`].
pub fn start_win_shift_s_hook() {
    imp::start_win_shift_s_hook()
}

/// True once after each Win+Shift+S that the hook swallowed.
pub fn take_win_shift_s() -> bool {
    imp::take_win_shift_s()
}

/// Takes the named start lock for this user (`Local\\...` names are per session). True for the one process that holds
/// it; Windows frees it when that process ends, or when [`release_start_lock`] is called first.
pub fn take_start_lock(name: &str) -> bool {
    imp::take_start_lock(name)
}

/// Gives the start lock up now, so the copy that takes over can start at once.
pub fn release_start_lock() {
    imp::release_start_lock()
}

/// Ends this process at once (no DLL detach hang on a hand-over).
pub fn exit_now(code: u32) -> ! {
    imp::exit_now(code)
}

/// Waits up to `ms` milliseconds for process `pid` to end. True when it has ended (or cannot be watched).
pub fn wait_for_exit(pid: u32, ms: u32) -> bool {
    imp::wait_for_exit(pid, ms)
}

/// Moves a file to the Recycle Bin (undoable from there). True when it went.
pub fn recycle(path: &std::path::Path) -> bool {
    imp::recycle(path)
}

/// A user folder where Windows keeps it; None when Windows does not say.
pub fn known_folder(which: UserFolder) -> Option<std::path::PathBuf> {
    imp::known_folder(which)
}

/// Pop-up menus (the tray's) in Windows' theme (1), dark (2) or light (3).
pub fn set_menu_theme(mode: i32) {
    imp::set_menu_theme(mode)
}

/// Opens Windows' Cast panel (Win+K): show this screen on a TV or wireless display.
pub fn open_cast_panel() {
    imp::open_cast_panel()
}

/// Text in a picture (RGBA), line by line, with Windows' built-in text recognition.
pub fn recognise_text(rgba: &[u8], w: u32, h: u32) -> Result<String, String> {
    imp::recognise_text(rgba, w, h)
}

/// Opens a web address or a Windows settings link (ms-settings:...) with the system. True when the system took it.
pub fn open_uri(uri: &str) -> bool {
    imp::open_uri(uri)
}

/// Prints a picture (RGBA) fitted to the page after the print dialog; Ok(false) when the person cancels.
pub fn print_image(owner: isize, rgba: &[u8], w: u32, h: u32, title: &str) -> Result<bool, String> {
    imp::print_image(owner, rgba, w, h, title)
}

/// Opens the system Share panel for a picture file. `owner` is the window's `HWND` as `isize`.
pub fn share_file(owner: isize, path: &std::path::Path, title: &str) -> Result<(), String> {
    imp::share_file(owner, path, title)
}

/// Sets a log file for [`log_line`]. Lines are dropped until one is set.
pub fn set_log_file(path: std::path::PathBuf) {
    imp::set_log_file(path)
}

/// Appends a line (with the local time) to the log file set by [`set_log_file`].
pub fn log_line(line: &str) {
    imp::hook_log(line.to_string())
}

/// Year, month, day, hour, minute, second on the local clock.
pub fn local_time() -> [u16; 6] {
    imp::local_time()
}

/// Whole-window opacity (0..=255) of a top-level window. 255 removes the layered style.
pub fn set_window_alpha(hwnd: isize, alpha: u8) -> Result<(), String> {
    imp::set_window_alpha(hwnd, alpha)
}

/// Keeps the window below every other window, or returns it to normal. Does not move, resize or activate it.
pub fn set_always_on_bottom(hwnd: isize, on: bool) -> Result<(), String> {
    imp::set_always_on_bottom(hwnd, on)
}

/// Makes `owner` (0 = none) the owner window, so `hwnd` stays above it.
pub fn set_owner(hwnd: isize, owner: isize) -> Result<(), String> {
    imp::set_owner(hwnd, owner)
}

/// Portable answers for targets without the Windows calls (Android, macOS, Linux, wasm32). Nothing is captured,
/// nothing is hooked, and each call that would act on the OS says so.
#[cfg(not(windows))]
mod portable {
    use super::{ScreenRect, TopWindow, UserFolder, WindowList};

    const WINDOWS_ONLY: &str = "this is a Windows call and this platform has none";

    pub fn virtual_screen() -> ScreenRect {
        ScreenRect::default()
    }
    pub fn cursor_monitor() -> Option<((i32, i32), ScreenRect)> {
        None
    }
    pub fn list_windows(_: WindowList) -> Vec<TopWindow> {
        Vec::new()
    }
    pub fn file_types_changed() {}
    pub fn exclude_from_capture(_: isize) {}
    pub fn start_no_window_animations(_: &str) {}
    pub fn start_win_shift_s_hook() {}
    pub fn take_win_shift_s() -> bool {
        false
    }
    pub fn take_start_lock(_: &str) -> bool {
        true
    }
    pub fn release_start_lock() {}
    pub fn exit_now(code: u32) -> ! {
        std::process::exit(code as i32)
    }
    pub fn wait_for_exit(_: u32, _: u32) -> bool {
        true
    }
    pub fn recycle(path: &std::path::Path) -> bool {
        std::fs::remove_file(path).is_ok()
    }
    pub fn known_folder(_: UserFolder) -> Option<std::path::PathBuf> {
        None
    }
    pub fn set_menu_theme(_: i32) {}
    pub fn open_cast_panel() {}
    pub fn recognise_text(_: &[u8], _: u32, _: u32) -> Result<String, String> {
        Err(format!("text recognition: {WINDOWS_ONLY}"))
    }
    pub fn open_uri(_: &str) -> bool {
        false
    }
    pub fn print_image(_: isize, _: &[u8], _: u32, _: u32, _: &str) -> Result<bool, String> {
        Err(format!("printing: {WINDOWS_ONLY}"))
    }
    pub fn share_file(_: isize, _: &std::path::Path, _: &str) -> Result<(), String> {
        Err(format!("sharing: {WINDOWS_ONLY}"))
    }
    pub fn set_log_file(_: std::path::PathBuf) {}
    pub fn hook_log(_: String) {}
    pub fn local_time() -> [u16; 6] {
        [2000, 1, 1, 0, 0, 0]
    }
    pub fn set_window_alpha(_: isize, _: u8) -> Result<(), String> {
        Err(format!("window alpha: {WINDOWS_ONLY}"))
    }
    pub fn set_always_on_bottom(_: isize, _: bool) -> Result<(), String> {
        Err(format!("window order: {WINDOWS_ONLY}"))
    }
    pub fn set_owner(_: isize, _: isize) -> Result<(), String> {
        Err(format!("window owner: {WINDOWS_ONLY}"))
    }

    /// Grabs nothing: `new` is always `None`, so no grabber exists on this target.
    pub struct ScreenGrabber;

    impl ScreenGrabber {
        pub fn new(_: ScreenRect) -> Option<ScreenGrabber> {
            None
        }
        pub fn grab(&mut self, _: bool) -> Option<&[u8]> {
            None
        }
    }
}
