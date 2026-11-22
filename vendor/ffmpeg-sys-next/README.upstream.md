[![ffmpeg-sys-next on crates.io](https://img.shields.io/crates/v/ffmpeg-sys-next?cacheSeconds=3600)](https://crates.io/crates/ffmpeg-sys-next)
[![build](https://github.com/zmwangx/rust-ffmpeg-sys/actions/workflows/build.yml/badge.svg?branch=master)](https://github.com/zmwangx/rust-ffmpeg-sys/actions)

This is a fork of the abandoned [ffmpeg-sys](https://github.com/meh/rust-ffmpeg-sys) crate. You can find this crate as [ffmpeg-sys-next](https://crates.io/crates/ffmpeg-sys-next) on crates.io.

This crate contains low level bindings to FFmpeg. You're probably interested in the high level bindings instead: [ffmpeg-next](https://github.com/zmwangx/rust-ffmpeg).

A word on versioning: major and minor versions track major and minor versions of FFmpeg, e.g. 4.2.x of this crate has been updated to support the 4.2.x series of FFmpeg. Patch level is reserved for bug fixes of this crate and does not track FFmpeg patch versions.

## Feature flags

In addition to feature flags declared in `Cargo.toml`, this crate performs various compile-time version and feature detections and exposes the results in additional flags. These flags are briefly documented below; run `cargo build -vv` to view more details.

- `ffmpeg_<x>_<y>` flags (new in v4.3.2), e.g. `ffmpeg_4_4`, indicating the FFmpeg installation being compiled against is at least version `<x>.<y>`. Currently available:

  - `ffmpeg_3_0`
  - `ffmpeg_3_1`
  - `ffmpeg_3_2`
  - `ffmpeg_3_3`
  - `ffmpeg_3_4`
  - `ffmpeg_4_0`
  - `ffmpeg_4_1`
  - `ffmpeg_4_2`
  - `ffmpeg_4_3`
  - `ffmpeg_4_4`
  - `ffmpeg_5_0`
  - `ffmpeg_5_1`
  - `ffmpeg_6_0`
  - `ffmpeg_6_1`
  - `ffmpeg_7_0`
  - `ffmpeg_7_1`
  - `ffmpeg_8_0`
  - `ffmpeg_8_1`
  - `ffmpeg_9_0`
  - `ffmpeg_9_1`

- `avcodec_version_greater_than_<x>_<y>` (new in v4.3.2), e.g., `avcodec_version_greater_than_58_90`. The name should be self-explanatory.

- `ff_api_<feature>`, e.g. `ff_api_vaapi`, corresponding to whether their respective uppercase deprecation guards evaluate to true.

- `ff_api_<feature>_is_defined`, e.g. `ff_api_vappi_is_defined`, similar to above except these are enabled as long as the corresponding deprecation guards are defined.

## Rimelight changes

This copy is ffmpeg-sys-next 9.1.1 with the Rimelight changes below. `src/` is upstream's, unchanged. The Rimelight
changes are all in `build.rs` and `bindings/`; the crate's doc comment explains them in full.

- **Checked-in bindings.** `bindings/x86_64-pc-windows-msvc.rs` and `.cfg` replace bindgen and the FFmpeg headers, so no
  machine or CI runner needs clang or the FFmpeg source. Only x86_64 Windows has them.
- **Feature-gated linking.** `avutil` is always linked; every other library (avcodec, avformat, swresample, swscale,
  avdevice, avfilter) only when its cargo feature is on, so an app without `device` or `filter` does not need them.
- **DLL copy.** The DLLs of the linked libraries are copied next to the built programs (the profile folder, found as
  the parent of the nearest `build` folder above `OUT_DIR`, so it works for both Cargo layouts), and that folder is
  added as a native search path so `cargo run` and `cargo test` find them.
- **FFmpeg folder.** `FFMPEG_DIR` when set, else the first `ffmpeg/current` above `OUT_DIR` or above this crate. Each
  app fetches its build there: Space with `cargo xtask ffmpeg` (into the target folder), Media with
  `tools/get-ffmpeg.ps1`.
- **Pin check.** `PIN` in `build.rs` names the FFmpeg build this crate expects. If the folder's `PINNED_RELEASE.txt`
  says otherwise, the build warns and the folder should be fetched again.
- **Warning, not error, without FFmpeg.** A missing folder prints a warning; `cargo check` and clippy still work.
