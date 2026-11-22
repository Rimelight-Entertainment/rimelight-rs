# rimelight_platform

OS calls shared by Rimelight Media and Rimelight Space, behind one portable API.

- **Windows** (`src/win32.rs`, `cfg(windows)`): screen capture (GDI), the virtual screen and the cursor's monitor, the
  top-level window list, window alpha, z-order, owner and capture exclusion, the Win+Shift+S hook, the no-animation hook,
  the named start lock, Recycle Bin, known folders, print, Share, text recognition (WinRT), Explorer's file-type refresh,
  the menu theme, and the Cast panel.
- **Android** (`src/android.rs`, `cfg(target_os = "android")`): open a URL, read the activity's intent data, save a
  download to the Downloads folder (JNI).
- **Everywhere else**: the same functions answer empty (`None`, `false`, an empty list) or return an `Err` that names the
  platform. A screen grabber is `None`, so no capture exists there.

Unsafe code is denied crate-wide and allowed only in `win32` and `android`. Each unsafe block has a `// SAFETY:`
comment, and the audit list is at the top of each module.

## Public API (crate root)

| Function | Notes |
|---|---|
| `virtual_screen()`, `cursor_monitor()` | Physical pixels. |
| `list_windows(WindowList)` | Presets: `WindowList::capturable(keep)` (Media) and `WindowList::popout_owners(exclude)` (Space). |
| `exclude_from_capture(hwnd)`, `set_window_alpha(hwnd, u8)`, `set_always_on_bottom(hwnd, bool)`, `set_owner(hwnd, owner)` | `hwnd` is an `HWND` as `isize`. The last three return `Result<(), String>`. |
| `start_no_window_animations(class)` | Pass the window class the app sets. |
| `start_win_shift_s_hook()`, `take_win_shift_s()` | |
| `take_start_lock(name)`, `release_start_lock()` | `name` such as `Local\\RimelightMedia.Instance`. |
| `exit_now(code)`, `wait_for_exit(pid, ms)` | |
| `recycle(path)`, `known_folder(UserFolder)` | |
| `set_menu_theme(mode)`, `open_cast_panel()`, `file_types_changed()` | |
| `recognise_text(rgba, w, h)`, `print_image(...)`, `share_file(...)` | `Result`s carry the text for the person. |
| `open_uri(uri)` | |
| `set_log_file(path)`, `log_line(line)`, `local_time()` | Lines are dropped until a log file is set. |
| `ScreenGrabber::new(rect)`, `grab(cursor)` | |

Android: `rimelight_platform::android::{open_url, intent_data, save_download}`, each taking the `AndroidApp`.

## Using it

```toml
rimelight_platform = { path = "../../crates/rimelight_platform" }
```

Callers need no `cfg`. A Windows-only feature (a pop-out, a hotkey) is still `cfg`-gated by the app where it is wired.
