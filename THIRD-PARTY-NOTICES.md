# Third-party software in the Rimelight shared crates

These notices apply to any Rimelight app that ships the FFmpeg build from `tools/ffmpeg/` (Rimelight Media and Rimelight
Space). The crates in this repository do not contain FFmpeg; the vendored `vendor/ffmpeg-sys-next` only links to it.

- **FFmpeg** n9.0.2 (avcodec, avformat, avutil, swresample, swscale; avdevice and avfilter where an app enables them),
  LGPL 2.1 or later, built without the GPL parts from the options in `tools/ffmpeg/options.txt`, with the unmodified
  FFmpeg source by `tools/ffmpeg/build.sh`. Source: https://github.com/FFmpeg/FFmpeg/tree/n9.0.2. The licence text and
  the configure options that were used ship in the FFmpeg folder as `LICENSE.txt` and `BUILD-INFO.txt`. You may replace
  the DLLs with your own build of the same FFmpeg major version.
- **dav1d** 1.5.5 (AV1 decoding, inside avcodec), BSD 2-clause. Its licence text ships in the FFmpeg folder as
  `dav1d-LICENSE.txt`.
- **ffmpeg-sys-next** 9.1.1 (the vendored bindings in `vendor/ffmpeg-sys-next`), WTFPL. Upstream licence text is in
  `vendor/ffmpeg-sys-next/Cargo.toml`.
- **ffmpeg-next** (the high-level bindings apps use), WTFPL, from crates.io.
- Rust crates under the Rimelight shared crates' dependencies (see each crate on crates.io) are under MIT and/or
  Apache-2.0 unless their own notice says otherwise.
