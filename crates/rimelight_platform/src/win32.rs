//! Windows calls (Win32 through `windows-sys`, WinRT through `windows`). Each function is a checked wrapper: callers get
//! plain Rust values, a `Result` with the failure text, or `bool`/`Option`.
//!
//! Audit (unsafe blocks, by function; every one has a `// SAFETY:` comment at the block):
//! - `virtual_screen`: GetSystemMetrics (no preconditions).
//! - `title_of`, `bounds_of`, `is_cloaked`: GetWindowTextLengthW/GetWindowTextW into a sized buffer; DwmGetWindowAttribute
//!   into a RECT / u32 of the size the attribute takes.
//! - `each` (EnumWindows callback), `list_windows`: the callback's lparam is the `&mut Collect` alive for the call.
//! - `file_types_changed`: SHChangeNotify with no items.
//! - `keep_awake`: SetThreadExecutionState with flag bits only.
//! - `ScreenGrabber::new`, `grab`, `Drop`: GDI set-up, BitBlt and cursor drawing, the DIB section read after GdiFlush;
//!   every handle is released once in Drop (or on the failure path of `new`).
//! - `cursor_monitor`: GetCursorPos, MonitorFromPoint, GetMonitorInfoW with cbSize set.
//! - `take_start_lock`, `release_start_lock`: CreateMutexW with a NUL-terminated name; the handle is closed once.
//! - `exit_now`: TerminateProcess on the current process pseudo-handle.
//! - `wait_for_exit`, `close_snip_process`: OpenProcess, wait or query, CloseHandle.
//! - `recycle`: SHFileOperationW with a double-NUL-terminated path list that outlives the call.
//! - `known_folder`: SHGetKnownFolderPath, read up to the NUL, freed with CoTaskMemFree.
//! - `set_menu_theme`: LoadLibraryW + GetProcAddress by ordinal 135/136 (uxtheme, not in the public headers); the
//!   transmuted pointers are called with the signatures Windows 10 1903 and later use. A missing ordinal is skipped.
//! - `open_cast_panel`: SendInput with four well-formed keyboard inputs (only when the Settings link fails).
//! - `recognise_text`: WinRT calls on values this function owns; no raw pointers.
//! - `open_uri`: ShellExecuteW with NUL-terminated strings that outlive the call.
//! - `print_image`: PRINTDLGW zeroed and sized as required; the DC is used for one document and deleted; the buffers
//!   outlive every call that reads them.
//! - `share_file`: WinRT interop (`GetForWindow`, `ShowShareUIForWindow`) with our own top-level window.
//! - `exclude_from_capture`: SetWindowDisplayAffinity on a window handle (a stale one only makes the call fail).
//! - `start_no_window_animations`, `no_animation_proc`, `is_our_class`: SetWindowsHookExW with a static procedure,
//!   the CWPSTRUCT/WINDOWPOS reads for HC_ACTION, GetClassNameW into a 64-unit buffer, DwmSetWindowAttribute.
//! - `install_keyboard_hook`, `start_win_shift_s_hook`, `win_shift_s_proc`: SetWindowsHookExW / UnhookWindowsHookEx /
//!   SetWinEventHook with static procedures, and the message loop of the hook thread.
//! - `snip_window_event`: GetWindowThreadProcessId on the handle the system gave; GetTickCount64.
//! - `local_time`: GetLocalTime into a zeroed SYSTEMTIME.
//! - `set_window_alpha`, `set_always_on_bottom`, `set_owner`, `read_long`, `write_long`: GetWindowLongPtrW,
//!   SetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowPos on a window handle, with GetLastError to tell a 0 result
//!   from a failure.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};

use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWA_TRANSITIONS_FORCEDISABLED,
    DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK;
use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GWL_EXSTYLE, GWLP_HWNDPARENT, GetSystemMetrics, GetWindowLongPtrW, GetWindowLongW,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, HWND_BOTTOM, HWND_NOTOPMOST,
    IsIconic, IsWindowVisible, LWA_ALPHA, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
};

use crate::{ScreenRect, TopWindow, UserFolder, WindowList};

/// The virtual screen (every monitor's bounding box).
pub fn virtual_screen() -> ScreenRect {
    // SAFETY: GetSystemMetrics has no preconditions.
    unsafe {
        ScreenRect {
            x: GetSystemMetrics(SM_XVIRTUALSCREEN),
            y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            w: GetSystemMetrics(SM_CXVIRTUALSCREEN),
            h: GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

fn title_of(hwnd: HWND) -> String {
    // SAFETY: hwnd comes from EnumWindows; the buffer is sized from GetWindowTextLengthW plus the terminator.
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..got.max(0) as usize])
    }
}

fn bounds_of(hwnd: HWND) -> Option<ScreenRect> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: r is a valid RECT and the size matches DWMWA_EXTENDED_FRAME_BOUNDS's output.
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            (&mut r as *mut RECT).cast::<c_void>(),
            std::mem::size_of::<RECT>() as u32,
        )
    };
    (hr == 0 && r.right > r.left && r.bottom > r.top).then_some(ScreenRect {
        x: r.left,
        y: r.top,
        w: r.right - r.left,
        h: r.bottom - r.top,
    })
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked: u32 = 0;
    // SAFETY: cloaked is a valid u32, which is DWMWA_CLOAKED's output size.
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            (&mut cloaked as *mut u32).cast::<c_void>(),
            4,
        )
    };
    hr == 0 && cloaked != 0
}

/// The state EnumWindows hands to `each` through its LPARAM.
struct Collect {
    own_pid: u32,
    opts: WindowList,
    out: Vec<TopWindow>,
}

/// The EnumWindows callback: applies the rules of `WindowList` to one top-level window.
unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the &mut Collect passed to EnumWindows in list_windows, alive for the whole enumeration.
    let collect = unsafe { &mut *(lparam as *mut Collect) };
    if let Some(limit) = collect.opts.limit
        && collect.out.len() >= limit
    {
        return 0;
    }
    let id = hwnd as isize;
    if collect.opts.exclude == Some(id) {
        return 1;
    }
    // SAFETY: plain queries on a window handle EnumWindows gave us.
    let (visible, minimised, ex_style, pid) = unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        (
            IsWindowVisible(hwnd) != 0,
            IsIconic(hwnd) != 0,
            GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
            pid,
        )
    };
    if !visible {
        return 1;
    }
    let capturable = collect.opts.capturable_only;
    if capturable
        && (minimised
            || ex_style & WS_EX_TOOLWINDOW != 0
            || (pid == collect.own_pid && collect.opts.keep != Some(id)))
    {
        return 1;
    }
    let title = title_of(hwnd);
    if title.is_empty() || (capturable && is_cloaked(hwnd)) {
        return 1;
    }
    let rect = match bounds_of(hwnd) {
        Some(rect) => rect,
        None if capturable => return 1,
        None => ScreenRect::default(),
    };
    collect.out.push(TopWindow {
        hwnd: id,
        title,
        rect,
    });
    1
}

/// Visible top-level windows matching `opts`, front-most first.
pub fn list_windows(opts: WindowList) -> Vec<TopWindow> {
    let mut collect = Collect {
        own_pid: std::process::id(),
        opts,
        out: Vec::new(),
    };
    // SAFETY: the callback only uses lparam as &mut Collect, which outlives this call.
    unsafe {
        EnumWindows(Some(each), (&mut collect as *mut Collect) as LPARAM);
    }
    collect.out
}

/// Tells Explorer that file associations changed.
pub fn file_types_changed() {
    // SAFETY: SHCNE_ASSOCCHANGED takes no items.
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
}

/// Keeps the display and the system awake (`on`), or restores normal power management (`!on`). The state is per
/// thread: the call takes effect for the calling thread's execution state, so call it on the UI thread.
pub fn keep_awake(on: bool) {
    let flags = if on {
        ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
    } else {
        ES_CONTINUOUS
    };
    // SAFETY: SetThreadExecutionState takes only flag bits and no pointers. Its result (the previous state, or 0 on
    // failure) is not needed, so it is dropped.
    unsafe {
        SetThreadExecutionState(flags);
    }
}

use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC,
    SRCCOPY, SelectObject,
};
use windows_sys::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CURSOR_SHOWING, CURSORINFO, DI_NORMAL, DrawIconEx, GetCursorInfo, GetIconInfo, ICONINFO,
};

/// Copies screen pixels with GDI into one reusable bitmap.
pub struct ScreenGrabber {
    rect: ScreenRect,
    screen_dc: HDC,
    mem_dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
}

impl ScreenGrabber {
    pub fn new(rect: ScreenRect) -> Option<ScreenGrabber> {
        if rect.w <= 0 || rect.h <= 0 {
            return None;
        }
        // SAFETY: standard GDI set-up; every handle is checked and released in Drop (or below on failure).
        unsafe {
            let screen_dc = GetDC(std::ptr::null_mut());
            if screen_dc.is_null() {
                return None;
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: rect.w,
                biHeight: -rect.h, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                mem_dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if mem_dc.is_null() || bitmap.is_null() || bits.is_null() {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                if !mem_dc.is_null() {
                    DeleteDC(mem_dc);
                }
                ReleaseDC(std::ptr::null_mut(), screen_dc);
                return None;
            }
            let old = SelectObject(mem_dc, bitmap);
            Some(ScreenGrabber {
                rect,
                screen_dc,
                mem_dc,
                bitmap,
                old,
                bits: bits.cast(),
            })
        }
    }

    /// The screen area as BGRA rows (top-down, width * 4 bytes each; alpha undefined). `cursor` draws the pointer.
    pub fn grab(&mut self, cursor: bool) -> Option<&[u8]> {
        let r = self.rect;
        // SAFETY: the DCs and bitmap live as long as self; `bits` points to the DIB section of r.w * r.h * 4 bytes,
        // read only after GdiFlush finished drawing into it.
        unsafe {
            if BitBlt(
                self.mem_dc,
                0,
                0,
                r.w,
                r.h,
                self.screen_dc,
                r.x,
                r.y,
                SRCCOPY,
            ) == 0
            {
                return None;
            }
            if cursor {
                let mut ci: CURSORINFO = std::mem::zeroed();
                ci.cbSize = std::mem::size_of::<CURSORINFO>() as u32;
                if GetCursorInfo(&mut ci) != 0 && ci.flags & CURSOR_SHOWING != 0 {
                    let mut ii: ICONINFO = std::mem::zeroed();
                    let (hx, hy) = if GetIconInfo(ci.hCursor, &mut ii) != 0 {
                        if !ii.hbmMask.is_null() {
                            DeleteObject(ii.hbmMask);
                        }
                        if !ii.hbmColor.is_null() {
                            DeleteObject(ii.hbmColor);
                        }
                        (ii.xHotspot as i32, ii.yHotspot as i32)
                    } else {
                        (0, 0)
                    };
                    DrawIconEx(
                        self.mem_dc,
                        ci.ptScreenPos.x - r.x - hx,
                        ci.ptScreenPos.y - r.y - hy,
                        ci.hCursor,
                        0,
                        0,
                        0,
                        std::ptr::null_mut(),
                        DI_NORMAL,
                    );
                }
            }
            GdiFlush();
            Some(std::slice::from_raw_parts(
                self.bits,
                (r.w * r.h * 4) as usize,
            ))
        }
    }
}

impl Drop for ScreenGrabber {
    fn drop(&mut self) {
        // SAFETY: releases exactly what new() created, once.
        unsafe {
            SelectObject(self.mem_dc, self.old);
            DeleteObject(self.bitmap);
            DeleteDC(self.mem_dc);
            ReleaseDC(std::ptr::null_mut(), self.screen_dc);
        }
    }
}

/// The pointer position and the rectangle of the monitor it is on.
pub fn cursor_monitor() -> Option<((i32, i32), ScreenRect)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    // SAFETY: plain queries filling structs we own; cbSize is set as GetMonitorInfoW requires.
    unsafe {
        let mut p = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut p) == 0 {
            return None;
        }
        let mon = MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut info) == 0 {
            return None;
        }
        let r = info.rcMonitor;
        Some((
            (p.x, p.y),
            ScreenRect {
                x: r.left,
                y: r.top,
                w: r.right - r.left,
                h: r.bottom - r.top,
            },
        ))
    }
}

/// The start lock's handle (0 when this copy doesn't hold it).
static START_LOCK: AtomicIsize = AtomicIsize::new(0);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Takes the named lock (for example `Local\\RimelightMedia.Instance`). True for the one process that holds it.
pub fn take_start_lock(name: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name = wide(name);
    // SAFETY: a NUL-terminated name; the handle is kept in START_LOCK, or closed when another copy already owns it.
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if h.is_null() {
            return false;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(h);
            return false;
        }
        START_LOCK.store(h as isize, Ordering::SeqCst);
        true
    }
}

/// Gives the start lock up now, so the copy that takes over can start at once.
pub fn release_start_lock() {
    use windows_sys::Win32::Foundation::CloseHandle;
    let h = START_LOCK.swap(0, Ordering::SeqCst);
    if h != 0 {
        // SAFETY: the handle came from CreateMutexW in take_start_lock; the swap above makes sure it is closed once.
        unsafe {
            CloseHandle(h as _);
        }
    }
}

/// Ends this process at once. TerminateProcess skips the DLL detach of the audio and media threads, which could hang an
/// ordinary exit while a hand-over waits for this process to end.
pub fn exit_now(code: u32) -> ! {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
    // SAFETY: the current process pseudo-handle; terminating ourselves is the point.
    unsafe {
        TerminateProcess(GetCurrentProcess(), code);
    }
    std::process::exit(code as i32)
}

/// Waits up to `ms` for another process to end. True when it has ended (or can't be watched).
pub fn wait_for_exit(pid: u32, ms: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};
    // SYNCHRONIZE: the access right that lets a process wait on another's handle (0x00100000).
    const SYNCHRONIZE: u32 = 0x0010_0000;
    // SAFETY: the handle is checked, waited on and closed here.
    unsafe {
        let h = OpenProcess(SYNCHRONIZE, 0, pid);
        if h.is_null() {
            return true;
        }
        let done = WaitForSingleObject(h, ms) == WAIT_OBJECT_0;
        CloseHandle(h);
        done
    }
}

/// Ends `pid` when it is Windows' snipping overlay (ScreenClippingHost.exe). True when it was ended.
fn close_snip_process(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        QueryFullProcessImageNameW, TerminateProcess,
    };
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: the handle is checked and closed here; the name buffer and its length are passed as the call needs.
    unsafe {
        let h = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
            0,
            pid,
        );
        if h.is_null() {
            return false;
        }
        let named = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0;
        let killed = named && snip_exe(&buf[..len as usize]) && TerminateProcess(h, 0) != 0;
        CloseHandle(h);
        killed
    }
}

/// True when a full image path ends in ScreenClippingHost.exe.
fn snip_exe(path: &[u16]) -> bool {
    let p = String::from_utf16_lossy(path);
    Path::new(&p).file_name().is_some_and(|n| {
        n.to_string_lossy()
            .eq_ignore_ascii_case("ScreenClippingHost.exe")
    })
}

/// Moves a file to the Recycle Bin. True when it went (and nothing was aborted).
pub fn recycle(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
        SHFileOperationW,
    };
    // Double-NUL-terminated list of one path.
    let mut from: Vec<u16> = path.as_os_str().encode_wide().collect();
    from.extend_from_slice(&[0, 0]);
    // SAFETY: `from` outlives the call and is double-NUL-terminated as SHFileOperationW requires.
    unsafe {
        let mut op: SHFILEOPSTRUCTW = std::mem::zeroed();
        op.wFunc = FO_DELETE;
        op.pFrom = from.as_ptr();
        op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16;
        SHFileOperationW(&mut op) == 0 && op.fAnyOperationsAborted == 0
    }
}

/// A user folder, wherever Windows keeps it (OneDrive can move them).
pub fn known_folder(which: UserFolder) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt as _;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_Music, FOLDERID_Pictures, FOLDERID_Videos, SHGetKnownFolderPath,
    };
    let id = match which {
        UserFolder::Pictures => &FOLDERID_Pictures,
        UserFolder::Videos => &FOLDERID_Videos,
        UserFolder::Music => &FOLDERID_Music,
    };
    // SAFETY: SHGetKnownFolderPath allocates the string; it is read up to its NUL and freed with CoTaskMemFree.
    unsafe {
        let mut p: *mut u16 = std::ptr::null_mut();
        let hr = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut p);
        if hr != 0 || p.is_null() {
            if !p.is_null() {
                CoTaskMemFree(p.cast());
            }
            return None;
        }
        let len = (0..).take_while(|&i| *p.add(i) != 0).count();
        let path = std::ffi::OsString::from_wide(std::slice::from_raw_parts(p, len));
        CoTaskMemFree(p.cast());
        Some(path.into())
    }
}

/// Light or dark pop-up menus for this process: 1 follow Windows, 2 dark, 3 light. Uses uxtheme's
/// SetPreferredAppMode and FlushMenuThemes by ordinal (not in the public headers; Windows' own apps and Chromium do the
/// same). Does nothing when they are missing.
pub fn set_menu_theme(mode: i32) {
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    let name = wide("uxtheme.dll");
    // SAFETY: uxtheme stays loaded; the ordinals 135 (SetPreferredAppMode(int) -> int) and 136 (FlushMenuThemes())
    // have had these signatures since Windows 10 1903, and a missing one is skipped.
    unsafe {
        let lib = LoadLibraryW(name.as_ptr());
        if lib.is_null() {
            return;
        }
        if let Some(f) = GetProcAddress(lib, 135usize as *const u8) {
            let set: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(f);
            set(mode);
        }
        if let Some(f) = GetProcAddress(lib, 136usize as *const u8) {
            let flush: unsafe extern "system" fn() = std::mem::transmute(f);
            flush();
        }
    }
}

/// Opens Windows' Cast panel (Win+K). The Settings page runs in its own process, so nothing is sent into our window
/// (sending Win+K to our own window made the XAML cast picker overrun the stack). Falls back to the key chord.
pub fn open_cast_panel() {
    if open_uri("ms-settings-connectabledevices:devicediscovery") {
        return;
    }
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_LWIN,
    };
    let key = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let inputs = [
        key(VK_LWIN, false),
        key(b'K' as u16, false),
        key(b'K' as u16, true),
        key(VK_LWIN, true),
    ];
    // SAFETY: four well-formed keyboard inputs, sized as SendInput requires.
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

/// The text in a picture, line by line, with Windows' built-in text recognition (the languages of the user's profile).
pub fn recognise_text(rgba: &[u8], w: u32, h: u32) -> Result<String, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    let e = |e: windows::core::Error| e.message().to_string();
    let max = OcrEngine::MaxImageDimension().map_err(e)?;
    if w > max || h > max {
        return Err(format!(
            "the area is too large to read (at most {max} pixels a side)"
        ));
    }
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|_| "Windows has no text recognition for your languages (Settings > Time & language > Language: add the OCR feature)".to_string())?;
    let mut bgra = Vec::with_capacity(rgba.len());
    for p in rgba.as_chunks::<4>().0 {
        bgra.extend_from_slice(&[p[2], p[1], p[0], 255]);
    }
    let writer = DataWriter::new().map_err(e)?;
    writer.WriteBytes(&bgra).map_err(e)?;
    let buffer = writer.DetachBuffer().map_err(e)?;
    let bitmap =
        SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, w as i32, h as i32)
            .map_err(e)?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(e)?
        .join()
        .map_err(e)?;
    let lines = result.Lines().map_err(e)?;
    let mut out = Vec::new();
    for i in 0..lines.Size().map_err(e)? {
        out.push(lines.GetAt(i).map_err(e)?.Text().map_err(e)?.to_string());
    }
    Ok(out.join("\n"))
}

/// Opens a web address or a Windows settings link with the system. True when the system took it (a result above 32).
pub fn open_uri(uri: &str) -> bool {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let (verb, target) = (wide("open"), wide(uri));
    // SAFETY: both strings are NUL-terminated and outlive the call; a result above 32 means success.
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    r as isize > 32
}

/// Prints one picture, fitted to the page with a small margin, after Windows' print dialog. Ok(false) when the person
/// cancels. `owner` is the window's `HWND` as `isize`.
pub fn print_image(owner: isize, rgba: &[u8], w: u32, h: u32, title: &str) -> Result<bool, String> {
    use windows_sys::Win32::Graphics::Gdi::{
        GetDeviceCaps, HALFTONE, HORZRES, SetBrushOrgEx, SetStretchBltMode, StretchDIBits, VERTRES,
    };
    use windows_sys::Win32::Storage::Xps::{DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
    use windows_sys::Win32::UI::Controls::Dialogs::{
        PD_NOPAGENUMS, PD_NOSELECTION, PD_RETURNDC, PRINTDLGW, PrintDlgW,
    };
    // BGRA rows for GDI.
    let mut bgra = Vec::with_capacity(rgba.len());
    for p in rgba.as_chunks::<4>().0 {
        bgra.extend_from_slice(&[p[2], p[1], p[0], 255]);
    }
    let name = wide(title);
    // SAFETY: PRINTDLGW is zeroed and sized as PrintDlgW requires; the device context it returns is used for one
    // document and deleted; `bgra` and `name` outlive every call that reads them.
    unsafe {
        let mut pd: PRINTDLGW = std::mem::zeroed();
        pd.lStructSize = std::mem::size_of::<PRINTDLGW>() as u32;
        pd.hwndOwner = owner as HWND;
        pd.Flags = PD_RETURNDC | PD_NOPAGENUMS | PD_NOSELECTION;
        pd.nCopies = 1;
        if PrintDlgW(&mut pd) == 0 || pd.hDC.is_null() {
            return Ok(false);
        }
        let dc = pd.hDC;
        let (pw, ph) = (
            GetDeviceCaps(dc, HORZRES as i32),
            GetDeviceCaps(dc, VERTRES as i32),
        );
        let margin = (pw.min(ph) as f64 * 0.04) as i32;
        let (aw, ah) = (f64::from(pw - 2 * margin), f64::from(ph - 2 * margin));
        let scale = (aw / f64::from(w)).min(ah / f64::from(h));
        let (dw, dh) = ((f64::from(w) * scale) as i32, (f64::from(h) * scale) as i32);
        let (dx, dy) = ((pw - dw) / 2, (ph - dh) / 2);
        let doc = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: name.as_ptr(),
            lpszOutput: std::ptr::null(),
            lpszDatatype: std::ptr::null(),
            fwType: 0,
        };
        let mut ok = StartDocW(dc, &doc) > 0 && StartPage(dc) > 0;
        if ok {
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            };
            SetStretchBltMode(dc, HALFTONE);
            SetBrushOrgEx(dc, 0, 0, std::ptr::null_mut());
            ok = StretchDIBits(
                dc,
                dx,
                dy,
                dw,
                dh,
                0,
                0,
                w as i32,
                h as i32,
                bgra.as_ptr().cast(),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            ) != 0;
            ok &= EndPage(dc) > 0;
            ok &= EndDoc(dc) > 0;
        }
        DeleteDC(dc);
        if ok {
            Ok(true)
        } else {
            Err("the printer refused the page".into())
        }
    }
}

thread_local! {
    static SHARE_HOOK: std::cell::Cell<Option<(isize, i64)>> = const { std::cell::Cell::new(None) };
}

/// Opens Windows' Share panel for a picture file. The file is checked now (a clear error) and again inside the handler,
/// since Windows' file object stays on its thread.
pub fn share_file(owner: isize, path: &Path, title: &str) -> Result<(), String> {
    use windows::ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager};
    use windows::Foundation::TypedEventHandler;
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::RandomAccessStreamReference;
    use windows::Win32::UI::Shell::IDataTransferManagerInterop;
    use windows::core::{HSTRING, Ref, factory};
    let e = |e: windows::core::Error| e.message().to_string();
    let path = HSTRING::from(path);
    StorageFile::GetFileFromPathAsync(&path)
        .map_err(e)?
        .join()
        .map_err(e)?;
    let interop = factory::<DataTransferManager, IDataTransferManagerInterop>().map_err(e)?;
    let hwnd = windows::Win32::Foundation::HWND(owner as *mut _);
    // SAFETY: hwnd is our own top-level window.
    let manager: DataTransferManager = unsafe { interop.GetForWindow(hwnd) }.map_err(e)?;
    // One handler per window: drop the one an earlier share added.
    if let Some((w, token)) = SHARE_HOOK.with(|h| h.take())
        && w == owner
    {
        let _ = manager.RemoveDataRequested(token);
    }
    let title = HSTRING::from(title);
    let handler = TypedEventHandler::<DataTransferManager, DataRequestedEventArgs>::new(
        move |_, args: Ref<DataRequestedEventArgs>| {
            let args = args.ok()?;
            let data = args.Request()?.Data()?;
            data.Properties()?.SetTitle(&title)?;
            let file = StorageFile::GetFileFromPathAsync(&path)?.join()?;
            data.SetBitmap(&RandomAccessStreamReference::CreateFromFile(&file)?)?;
            Ok(())
        },
    );
    let token = manager.DataRequested(&handler).map_err(e)?;
    SHARE_HOOK.with(|h| h.set(Some((owner, token))));
    // SAFETY: as above.
    unsafe { interop.ShowShareUIForWindow(hwnd) }.map_err(e)
}

/// Keeps a window of ours out of screenshots and recordings. `hwnd` is an `HWND` as `isize`.
pub fn exclude_from_capture(hwnd: isize) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
    };
    // SAFETY: hwnd is one of our own live windows (from winit); a stale handle only makes the call fail.
    unsafe {
        SetWindowDisplayAffinity(hwnd as HWND, WDA_EXCLUDEFROMCAPTURE);
    }
}

/// The window class that the no-animation hook matches, as UTF-16 with a NUL. Set once by start_no_window_animations.
static NO_ANIMATION_CLASS: OnceLock<Vec<u16>> = OnceLock::new();
/// The thread-local window-message hook, once installed (0 when none).
static SHOW_HOOK: AtomicIsize = AtomicIsize::new(0);

/// Watches window messages on the calling (UI) thread: just before a window of `window_class` is made visible, Windows'
/// open and close animations are turned off for it. At creation (a CBT hook) DWM ignored the setting: tested, every snip
/// zoomed, so the hook works on the show.
pub fn start_no_window_animations(window_class: &str) {
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_CALLWNDPROC};
    let _ = NO_ANIMATION_CLASS.set(wide(window_class));
    if SHOW_HOOK.load(Ordering::SeqCst) != 0 {
        return;
    }
    // SAFETY: a thread-local hook (no module handle needed) whose procedure is a static fn living for the process.
    let hook = unsafe {
        SetWindowsHookExW(
            WH_CALLWNDPROC,
            Some(no_animation_proc),
            std::ptr::null_mut(),
            GetCurrentThreadId(),
        )
    };
    if hook.is_null() {
        hook_log(
            "could not watch window messages: our windows keep Windows' open animation".into(),
        );
        return;
    }
    SHOW_HOOK.store(hook as isize, Ordering::SeqCst);
}

/// The message hook: runs before the window procedure for every message sent on the UI thread. Only a show of one of
/// our windows does anything.
unsafe extern "system" fn no_animation_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CWPSTRUCT, CallNextHookEx, GetClassNameW, HC_ACTION, SWP_SHOWWINDOW, WINDOWPOS,
        WM_SHOWWINDOW, WM_WINDOWPOSCHANGING,
    };
    if code == HC_ACTION as i32 && lparam != 0 {
        // SAFETY: for HC_ACTION, lparam points to a CWPSTRUCT that Windows keeps alive for the call.
        let m = unsafe { &*(lparam as *const CWPSTRUCT) };
        let showing = match m.message {
            WM_SHOWWINDOW => m.wParam != 0,
            // SAFETY: for WM_WINDOWPOSCHANGING, lParam points to the WINDOWPOS being applied.
            WM_WINDOWPOSCHANGING => {
                m.lParam != 0
                    && unsafe { (*(m.lParam as *const WINDOWPOS)).flags } & SWP_SHOWWINDOW != 0
            }
            _ => false,
        };
        if showing && !m.hwnd.is_null() {
            let mut class = [0u16; 64];
            // SAFETY: the buffer holds 64 u16s, as passed; GetClassNameW NUL-terminates what it writes.
            let n = unsafe { GetClassNameW(m.hwnd, class.as_mut_ptr(), class.len() as i32) };
            // SAFETY: `class` is NUL-terminated (zeroed, and GetClassNameW writes at most 63 characters).
            if n > 0 && unsafe { is_our_class(class.as_ptr()) } {
                let on: i32 = 1;
                // SAFETY: m.hwnd is the window being shown; `on` is a BOOL-sized value that lives for the call.
                unsafe {
                    DwmSetWindowAttribute(
                        m.hwnd,
                        DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                        (&on as *const i32).cast::<c_void>(),
                        4,
                    );
                }
            }
        }
    }
    // SAFETY: passes the message on to the next hook, as the hook contract requires.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// True when a class name (a NUL-terminated UTF-16 string; null or an atom below 0x10000 never match) is the window
/// class set by start_no_window_animations.
///
/// # Safety
/// `class` must be null, an atom, or point to a NUL-terminated UTF-16 string.
unsafe fn is_our_class(class: *const u16) -> bool {
    if (class as usize) <= 0xFFFF {
        return false;
    }
    let Some(want) = NO_ANIMATION_CLASS.get() else {
        return false;
    };
    for (i, &unit) in want.iter().enumerate() {
        // SAFETY: the string is NUL-terminated and read only up to its NUL (a mismatch stops the loop there); `want`
        // ends with the NUL, so the loop never reads past the terminator.
        if unsafe { *class.add(i) } != unit {
            return false;
        }
    }
    true
}

/// The Win+Shift+S hook's module and the handle of the keyboard hook now installed (0 when none).
static HOOK_MODULE: AtomicIsize = AtomicIsize::new(0);
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static HOOK_STARTED: AtomicBool = AtomicBool::new(false);
static WIN_S_PENDING: AtomicBool = AtomicBool::new(false);
static SWALLOW_S_UP: AtomicBool = AtomicBool::new(false);
static WIN_HELD: AtomicBool = AtomicBool::new(false);
static SHIFT_HELD: AtomicBool = AtomicBool::new(false);
/// Tick count (GetTickCount64) of the last foreground-driven keyboard hook re-install; 0 when none yet.
static LAST_FOREGROUND_REINSTALL_MS: AtomicU64 = AtomicU64::new(0);

/// Installs the keyboard hook at the front of the chain, replacing ours. Low-level hooks run newest first, so a hook
/// another program installed later can let Win+Shift+S reach Windows first; installing again puts ours first.
fn install_keyboard_hook(reason: &str) -> bool {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    };
    let module = HOOK_MODULE.load(Ordering::SeqCst);
    // SAFETY: the hook procedure is a static fn; the module handle is the process's own.
    let fresh =
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(win_shift_s_proc), module as _, 0) };
    if fresh.is_null() {
        // SAFETY: GetLastError reads this thread's last error code.
        let err = unsafe { GetLastError() };
        hook_log(format!(
            "keyboard hook install FAILED ({reason}): error {err}"
        ));
        return false;
    }
    let old = HOOK_HANDLE.swap(fresh as isize, Ordering::SeqCst);
    if old != 0 {
        // SAFETY: `old` was returned by SetWindowsHookExW earlier and is unhooked once.
        unsafe {
            UnhookWindowsHookEx(old as _);
        }
    }
    true
}

/// Starts the Win+Shift+S hooks on their own thread, with a message loop: the keyboard hook, and window-event hooks that
/// catch Windows' snipping overlay the moment it shows.
pub fn start_win_shift_s_hook() {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
    };
    use windows_sys::Win32::UI::Accessibility::SetWinEventHook;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND, GetMessageW, MSG,
        TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
    };
    if HOOK_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        // SAFETY: the module handle is the process's own; the thread handle is this thread's pseudo-handle.
        unsafe {
            // Windows drops a low-level hook that answers too slowly: this thread gets the top priority it can have.
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
            HOOK_MODULE.store(
                GetModuleHandleW(std::ptr::null()) as isize,
                Ordering::SeqCst,
            );
        }
        if !install_keyboard_hook("startup") {
            HOOK_STARTED.store(false, Ordering::SeqCst);
            return;
        }
        // Windows' snipping overlay shows (and takes focus) as a window of ScreenClippingHost.exe: these events come
        // here as it happens, so there is no poll. Out-of-context, so the callbacks run on this thread's loop.
        for event in [EVENT_SYSTEM_FOREGROUND, EVENT_OBJECT_SHOW] {
            // SAFETY: the callback is a static fn that lives for the whole process.
            unsafe {
                SetWinEventHook(
                    event,
                    event,
                    std::ptr::null_mut(),
                    Some(snip_window_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                );
            }
        }
        // SAFETY: the loop only dispatches messages to the hooks installed above.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
}

/// A window-event hook: when a window of the snipping overlay (ScreenClippingHost.exe) shows or takes focus, it is
/// closed, the snip starts here (take_win_shift_s), and the keyboard hook goes back to the front.
unsafe extern "system" fn snip_window_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    use windows_sys::Win32::System::SystemInformation::GetTickCount64;
    use windows_sys::Win32::UI::WindowsAndMessaging::{EVENT_SYSTEM_FOREGROUND, OBJID_WINDOW};
    if hwnd.is_null() || id_object != OBJID_WINDOW {
        return;
    }
    if event == EVENT_SYSTEM_FOREGROUND {
        // Another window took focus: our keyboard hook may have fallen behind hooks installed since, so put it back at
        // the front. At most once per 2 s (leading edge; no timer).
        // SAFETY: a plain tick-count query.
        let now = unsafe { GetTickCount64() };
        let last = LAST_FOREGROUND_REINSTALL_MS.load(Ordering::SeqCst);
        if last == 0 || now.saturating_sub(last) >= 2000 {
            LAST_FOREGROUND_REINSTALL_MS.store(now, Ordering::SeqCst);
            install_keyboard_hook("foreground window changed");
        }
    }
    let mut pid = 0u32;
    // SAFETY: a plain query on a window handle Windows gave us; `pid` is a valid out value.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 || !close_snip_process(pid) {
        return;
    }
    WIN_S_PENDING.store(true, Ordering::SeqCst);
    hook_log("ScreenClippingHost killed, snipping here (missed by hook)".into());
    install_keyboard_hook("after ScreenClippingHost was killed");
}

/// Swallows Win+Shift+S and flags it for the app to snip. The Win and Shift states are tracked here from the hook's own
/// events (the async key state lags behind them). The Win key-up is passed on: Shift was pressed in between, so the
/// Start menu stays shut.
unsafe extern "system" fn win_shift_s_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LSHIFT, VK_LWIN, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };
    /// KBDLLHOOKSTRUCT.flags bit set for keys sent by SendInput or another program (not the user's keyboard).
    const LLKHF_INJECTED: u32 = 0x10;
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, lparam points to a KBDLLHOOKSTRUCT that Windows keeps alive for the call.
        let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        // Injected keys are not the user's: they change no state here and are never swallowed.
        if kb.flags & LLKHF_INJECTED == 0 {
            let msg = wparam as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            let vk = kb.vkCode;
            if vk == VK_LWIN as u32 || vk == VK_RWIN as u32 {
                WIN_HELD.store(down, Ordering::SeqCst);
            } else if vk == VK_LSHIFT as u32 || vk == VK_RSHIFT as u32 || vk == VK_SHIFT as u32 {
                SHIFT_HELD.store(down, Ordering::SeqCst);
            } else if vk == b'S' as u32 {
                if down {
                    // Cross-check against the system key state, so a Win or Shift press this hook missed (or saw late)
                    // does not break Win+Shift+S. Only read here, on S, not on every key.
                    // SAFETY: plain key-state queries.
                    let sys_win = unsafe {
                        GetAsyncKeyState(VK_LWIN as i32) < 0 || GetAsyncKeyState(VK_RWIN as i32) < 0
                    };
                    // SAFETY: as above.
                    let sys_shift = unsafe { GetAsyncKeyState(VK_SHIFT as i32) < 0 };
                    let win = WIN_HELD.load(Ordering::SeqCst) || sys_win;
                    let shift = SHIFT_HELD.load(Ordering::SeqCst) || sys_shift;
                    if win && shift {
                        SWALLOW_S_UP.store(true, Ordering::SeqCst);
                        WIN_S_PENDING.store(true, Ordering::SeqCst);
                        return 1;
                    }
                } else if up && SWALLOW_S_UP.swap(false, Ordering::SeqCst) {
                    return 1;
                }
            }
        }
    }
    // SAFETY: passes the event on to the next hook, as the hook contract requires.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

/// True once after each Win+Shift+S.
pub fn take_win_shift_s() -> bool {
    WIN_S_PENDING.swap(false, Ordering::SeqCst)
}

/// Year, month, day, hour, minute, second on the local clock.
pub fn local_time() -> [u16; 6] {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime fills the SYSTEMTIME it is given.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    [t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond]
}

/// The log file set by `set_log_file`. Lines are dropped while it is unset.
static LOG_FILE: OnceLock<PathBuf> = OnceLock::new();

/// Sets the log file for `hook_log`. Set it once, before the first line.
pub fn set_log_file(path: PathBuf) {
    let _ = LOG_FILE.set(path);
}

/// Appends a line to the log file. One logger thread takes the lines from a channel, so the caller (the keyboard hook
/// included) never waits for the disk.
pub fn hook_log(line: String) {
    use std::sync::mpsc;
    static TX: OnceLock<mpsc::Sender<String>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            use std::io::Write as _;
            for line in rx {
                let Some(path) = LOG_FILE.get() else {
                    continue;
                };
                let t = local_time();
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = writeln!(
                        f,
                        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {line}",
                        t[0], t[1], t[2], t[3], t[4], t[5]
                    );
                }
                trim_log(path);
            }
        });
        tx
    });
    let _ = tx.send(line);
}

/// Keeps the log near 200 KB: once it grows past that, the oldest lines go and the newest 150 KB stay.
fn trim_log(path: &Path) {
    const CAP: u64 = 200 * 1024;
    const KEEP: usize = 150 * 1024;
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if meta.len() <= CAP {
        return;
    }
    let Ok(data) = std::fs::read(path) else {
        return;
    };
    let start = data.len().saturating_sub(KEEP);
    // Start the kept part at a line boundary so no line is cut in half.
    let cut = data[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(data.len(), |p| start + p + 1);
    let _ = std::fs::write(path, &data[cut..]);
}

/// Whole-window opacity (0..=255) through WS_EX_LAYERED and SetLayeredWindowAttributes(LWA_ALPHA). 255 removes the
/// layered style. Errors carry the Win32 error code.
pub fn set_window_alpha(hwnd: isize, alpha: u8) -> Result<(), String> {
    let style = read_long(hwnd, GWL_EXSTYLE)? as u32;
    if alpha == u8::MAX {
        if style & WS_EX_LAYERED != 0 {
            write_long(hwnd, GWL_EXSTYLE, (style & !WS_EX_LAYERED) as isize)?;
        }
        return Ok(());
    }
    if style & WS_EX_LAYERED == 0 {
        write_long(hwnd, GWL_EXSTYLE, (style | WS_EX_LAYERED) as isize)?;
    }
    // SAFETY: sets the layered alpha of the window `hwnd` (which now has WS_EX_LAYERED); no pointers are passed.
    let ok = unsafe { SetLayeredWindowAttributes(hwnd as HWND, 0, alpha, LWA_ALPHA) };
    if ok == 0 {
        return Err(failed("SetLayeredWindowAttributes"));
    }
    Ok(())
}

/// Keeps the window below every other window (HWND_BOTTOM with SWP_NOMOVE, SWP_NOSIZE and SWP_NOACTIVATE), or returns it
/// to normal (HWND_NOTOPMOST).
pub fn set_always_on_bottom(hwnd: isize, on: bool) -> Result<(), String> {
    let after = if on { HWND_BOTTOM } else { HWND_NOTOPMOST };
    // SAFETY: changes only the z-order of the window `hwnd` (no move, no resize, no activation); no pointers are passed.
    let ok = unsafe {
        SetWindowPos(
            hwnd as HWND,
            after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
    if ok == 0 {
        return Err(failed("SetWindowPos"));
    }
    Ok(())
}

/// Makes `owner` (0 = none) the owner window, so `hwnd` stays above it (GWLP_HWNDPARENT).
pub fn set_owner(hwnd: isize, owner: isize) -> Result<(), String> {
    write_long(hwnd, GWLP_HWNDPARENT, owner)
}

/// The message for a failed call, with the Win32 error code that was set.
fn failed(what: &str) -> String {
    format!("{what} failed (Win32 error {})", last_error())
}

/// The calling thread's last Win32 error code.
fn last_error() -> u32 {
    use windows_sys::Win32::Foundation::GetLastError;
    // SAFETY: GetLastError reads this thread's last-error value and takes no pointers.
    unsafe { GetLastError() }
}

/// Reads a window long. A 0 result is only an error when the last-error code was set by the call.
fn read_long(hwnd: isize, index: i32) -> Result<isize, String> {
    use windows_sys::Win32::Foundation::SetLastError;
    // SAFETY: SetLastError only sets this thread's last-error value and takes no pointers.
    unsafe { SetLastError(0) };
    // SAFETY: GetWindowLongPtrW reads one field of the window `hwnd`; a stale handle makes it fail, which is checked below.
    let value = unsafe { GetWindowLongPtrW(hwnd as HWND, index) };
    if value == 0 && last_error() != 0 {
        return Err(failed("GetWindowLongPtr"));
    }
    Ok(value)
}

/// Writes a window long. The previous value is 0 for a normal write, so only the last-error code decides failure.
fn write_long(hwnd: isize, index: i32, value: isize) -> Result<(), String> {
    use windows_sys::Win32::Foundation::SetLastError;
    // SAFETY: SetLastError only sets this thread's last-error value and takes no pointers.
    unsafe { SetLastError(0) };
    // SAFETY: SetWindowLongPtrW changes one field of the window `hwnd`; a stale handle makes it fail, which is checked below.
    let previous = unsafe { SetWindowLongPtrW(hwnd as HWND, index, value) };
    if previous == 0 && last_error() != 0 {
        return Err(failed("SetWindowLongPtr"));
    }
    Ok(())
}
