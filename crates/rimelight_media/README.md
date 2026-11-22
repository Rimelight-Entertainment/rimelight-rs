# rimelight_media

Video and audio for the Rimelight apps: read a file's details, take a poster frame, and play a video with sound.
The engine adds subtitles (text and picture), playback speed, and trimming without re-encoding.

## Platforms

Windows only for now. FFmpeg decodes and cpal plays the sound. Other platforms compile the same public API, and every
call returns `MediaError::Unsupported`.

FFmpeg is the shared build from `tools/ffmpeg` (used by `ffmpeg-next` 9.1 with only the codec, format, resampling
and scaling features; no device or filter libraries). Each app patches `ffmpeg-sys-next` to the copy vendored in
`vendor/`, so building needs neither clang nor FFmpeg headers.

## Cargo features

- `slint` (off by default): frames are stored as `slint::SharedPixelBuffer<Rgba8Pixel>`. The engine writes decoded
  frames straight into it (no copy into a Vec), and `Rgba::into_slint_image()` hands the picture to Slint with no
  copy. The Media app and Space enable it. Without it, `Rgba::pixels` is a `Vec<u8>`.
- Read pixels with `Rgba::bytes()` in both states. Only code that uses `pixels` directly, or `into_slint_image`, is
  feature-specific.

## API

Public API (the same on every platform, and the one Rimelight Space uses):

- `probe(path) -> Result<MediaInfo, MediaError>`: size, length, sound and MIME type.
- `poster(path, fraction, max_side) -> Result<Rgba, MediaError>`: one frame near a fraction of the video.
- `Player::open(path, PlayerOptions) -> Result<Player, MediaError>`: play, pause, seek, volume, mute, loop,
  output size, position, length, `has_audio`, `ended`, `take_frame` (RGBA `VideoFrame`), `error`.
- Types: `MediaInfo`, `PlayerOptions`, `Rgba`, `VideoFrame`, `MediaError`.

Windows only, the engine the Media app uses: `engine::player` (`probe` with tracks, titles and tags, `Player` with
audio track choice, subtitles, speed, `release` and `check_output`), `engine::subs`, `engine::bitmap_sub`,
`engine::stretch` (WSOLA time-stretching), and `engine::export::trim`.

## Tests

`cargo test -p rimelight_media` on Windows runs the tests against `tests/fixtures/clip.mp4` (2 s, 160x90, AAC stereo).
They need no audio device.
