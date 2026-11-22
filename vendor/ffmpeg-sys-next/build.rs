//! Build script for ffmpeg-sys-next with committed bindings (CV-V.2, decision 0046), shared by the Rimelight apps
//! (Rimelight Space and Rimelight Media).
//!
//! Upstream runs bindgen (clang) on the FFmpeg headers and a C feature check. Their whole output for one target was
//! recorded once and is checked in under `bindings/`:
//! - `<target>.rs`: the bindings, copied to `OUT_DIR/bindings.rs` (upstream's `src/lib.rs` includes that file);
//! - `<target>.cfg`: every `cargo:` line upstream printed except linking (cfg flags, check-cfg lines and the
//!   `DEP_FFMPEG_*` metadata `ffmpeg-next` reads), printed again here.
//!
//! Linking: the FFmpeg import libraries and DLLs come from the pinned FFmpeg build in `<folder>/current`, where
//! `<folder>` is `FFMPEG_DIR` when it is set, else the first `ffmpeg/` folder above `OUT_DIR` or above this crate
//! (an app's `ffmpeg/current` in its repo, or `<target dir>/ffmpeg/current`). Each app fetches it with its own
//! command (see the search order below).
//! Only `avutil` and the libraries whose cargo feature is on are linked, so an app without `device` or `filter`
//! does not need avdevice or avfilter. The DLLs of those libraries are copied into the profile folder
//! (`target/debug` or `target/release`), next to the app's exe, so it starts when run directly; that folder is also
//! reported as a native search path, because cargo puts search paths inside the profile folder (and only those) on
//! `PATH` for `cargo run` and `cargo test`. Test binaries live in per-unit folders under `build/`, so this is how
//! they find the DLLs.
//! A missing FFmpeg folder is only a warning: `cargo check` and clippy do not link.
//!
//! Recording the files again (new FFmpeg version or a new target): build upstream ffmpeg-sys-next once with
//! `FFMPEG_DIR` and `LIBCLANG_PATH` set, then copy `out/bindings.rs` and the `cargo:` lines of its build output.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The pinned FFmpeg build, `<release>/<asset>`, as `tools/ffmpeg/pin.txt` records it (`release` and `asset` lines).
/// A pin change updates this constant, `pin.txt`, and each app's fetch command together.
const PIN: &str = "ffmpeg-n9.0.2-r1/rimelight-ffmpeg-n9.0.2-r1-win64-lgpl-shared.zip";

/// The libraries upstream links, in its order.
const LIBS: &[&str] = &[
    "avcodec",
    "avdevice",
    "avfilter",
    "avformat",
    "avutil",
    "swresample",
    "swscale",
];

fn main() {
    let target = env::var("TARGET").expect("TARGET is set by cargo");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("set by cargo"));
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");

    let bindings = manifest.join("bindings").join(format!("{target}.rs"));
    let cfg = manifest.join("bindings").join(format!("{target}.cfg"));
    if !bindings.exists() || !cfg.exists() {
        panic!(
            "FFmpeg bindings are checked in for x86_64-pc-windows-msvc only; {target} has none. Video plays on \
             Windows first (decision 0046); see this build script's doc comment to record another target."
        );
    }
    println!("cargo:rerun-if-changed={}", bindings.display());
    println!("cargo:rerun-if-changed={}", cfg.display());
    fs::copy(&bindings, out_dir.join("bindings.rs")).expect("copy the bindings to OUT_DIR");
    for line in fs::read_to_string(&cfg).expect("read the recorded build output").lines() {
        if line.starts_with("cargo:") {
            println!("{line}");
        }
    }

    // Only the libraries whose feature is on (plus avutil, which everything needs).
    let linked: Vec<&str> = LIBS
        .iter()
        .copied()
        .filter(|lib| {
            *lib == "avutil" || env::var_os(format!("CARGO_FEATURE_{}", lib.to_ascii_uppercase())).is_some()
        })
        .collect();

    match ffmpeg_dir(&out_dir, &manifest) {
        Some(dir) => {
            check_pin(&dir);
            println!("cargo:rustc-link-search=native={}", dir.join("lib").display());
            if let Some(profile) = copy_dlls(&dir.join("bin"), &out_dir, &linked) {
                println!("cargo:rustc-link-search=native={}", profile.display());
            }
        }
        None => println!(
            "cargo:warning=FFmpeg is not set up: run your app's FFmpeg fetch (Space: `cargo xtask ffmpeg`; Media: \
             `tools/get-ffmpeg.ps1`); check and clippy work without it"
        ),
    }
    for lib in &linked {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
}

/// The FFmpeg folder (the one holding `lib/`, `bin/`): `FFMPEG_DIR` if set, else the first `ffmpeg/current` found
/// above `OUT_DIR` (the target folder, or an app's repo when the target folder sits inside it) or above this crate.
fn ffmpeg_dir(out_dir: &Path, manifest: &Path) -> Option<PathBuf> {
    if let Some(dir) = env::var_os("FFMPEG_DIR") {
        let dir = PathBuf::from(dir);
        return dir.join("lib").is_dir().then_some(dir);
    }
    out_dir
        .ancestors()
        .chain(manifest.ancestors())
        .map(|a| a.join("ffmpeg").join("current"))
        .find(|candidate| candidate.join("lib").is_dir())
}

/// Warns when the folder was fetched for another pin than `PIN` (`PINNED_RELEASE.txt`, written by the fetch command).
fn check_pin(dir: &Path) {
    let Ok(found) = fs::read_to_string(dir.join("PINNED_RELEASE.txt")) else { return };
    let found = found.trim();
    if found != PIN {
        println!(
            "cargo:warning=the FFmpeg folder {} holds {found}, but this build expects {PIN}; re-fetch it (your app's \
             FFmpeg fetch command)",
            dir.display()
        );
    }
}

/// The profile folder (`target/debug`, `target/release`, or `target/<triple>/<profile>`): the parent of the nearest
/// ancestor of `OUT_DIR` named `build`. Build-script output sits at `<profile>/build/<pkg>-<hash>/out`, or one level
/// deeper with newer Cargo (`<pkg>/<hash>/out`); this finds the same folder for both.
fn profile_dir(out_dir: &Path) -> Option<PathBuf> {
    let build = out_dir.ancestors().find(|a| a.file_name().and_then(|n| n.to_str()) == Some("build"))?;
    build.parent().map(Path::to_path_buf)
}

/// Copies the DLLs of the linked libraries (`avcodec-63.dll` belongs to `avcodec`: the name before the dash) into the
/// profile folder, when they are not there yet or differ in size. Returns the profile folder when it holds any of them.
fn copy_dlls(bin: &Path, out_dir: &Path, linked: &[&str]) -> Option<PathBuf> {
    let profile = profile_dir(out_dir)?;
    let entries = fs::read_dir(bin).ok()?;
    let dlls: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dll")))
        .filter(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let lib = stem.split('-').next().unwrap_or_default();
            linked.contains(&lib)
        })
        .collect();
    for dll in &dlls {
        let Some(name) = dll.file_name() else { continue };
        let dest = profile.join(name);
        let same = match (fs::metadata(dll), fs::metadata(&dest)) {
            (Ok(a), Ok(b)) => a.len() == b.len(),
            _ => false,
        };
        if !same && let Err(err) = fs::copy(dll, &dest) {
            println!("cargo:warning=could not copy {}: {err}", dll.display());
        }
    }
    (!dlls.is_empty()).then_some(profile)
}
