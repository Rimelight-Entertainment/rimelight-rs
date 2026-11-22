//! Video and audio: read a file's details, take a poster frame, and play a video with sound (with subtitles,
//! playback speed, and trimming for the Media app through `engine`).
//!
//! The real backend (FFmpeg for decoding, cpal for sound) is Windows only: decision 0046 for Space, and the shared
//! FFmpeg build in `tools/ffmpeg`. Other platforms (wasm, Android) get the same API from `stub`, and every call
//! returns `MediaError::Unsupported`.
//!
//! Frames are RGBA: four bytes per pixel, rows packed with no padding.
//!
//! Pixel storage (`Pixels`) depends on the optional `slint` feature (off by default; Media and Space enable it):
//! - feature off: `Pixels` is `Vec<u8>`. Read bytes with `Rgba::bytes()`.
//! - feature on: `Pixels` is `slint::SharedPixelBuffer<slint::Rgba8Pixel>`. The engine writes decoded frames straight
//!   into it (no copy into a Vec), and `Rgba::into_slint_image()` hands it to Slint without copying.
//!
//! Callers that only read pixels should use `Rgba::bytes()` and compile in both states.

use std::fmt;
#[cfg(windows)]
use std::path::Path;

#[cfg(windows)]
pub mod engine;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::{Player, poster, probe};

#[cfg(all(test, windows))]
mod tests;

/// What `probe` reads from a video file.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// Picture width in pixels.
    pub width: u32,
    /// Picture height in pixels.
    pub height: u32,
    /// Length in milliseconds.
    pub duration_ms: u64,
    /// True when the file has a sound track.
    pub has_audio: bool,
    /// "video/mp4", "video/quicktime", "video/webm" or "video/x-matroska" (from the container).
    pub mime: &'static str,
}

/// Pixel storage of an `Rgba`: `Vec<u8>` without the `slint` feature, `slint::SharedPixelBuffer<Rgba8Pixel>` with it.
#[cfg(feature = "slint")]
pub type Pixels = slint::SharedPixelBuffer<slint::Rgba8Pixel>;
/// Pixel storage of an `Rgba`: `Vec<u8>` without the `slint` feature, `slint::SharedPixelBuffer<Rgba8Pixel>` with it.
#[cfg(not(feature = "slint"))]
pub type Pixels = Vec<u8>;

/// A zeroed picture of the given size in the storage of the current feature state.
#[cfg(feature = "slint")]
pub(crate) fn new_pixels(width: u32, height: u32) -> Pixels {
    Pixels::new(width, height)
}

#[cfg(not(feature = "slint"))]
pub(crate) fn new_pixels(width: u32, height: u32) -> Pixels {
    vec![0u8; width as usize * height as usize * 4]
}

/// The bytes of a picture for writing (the engine copies decoded frames in here).
#[cfg(all(windows, feature = "slint"))]
pub(crate) fn pixel_bytes_mut(pixels: &mut Pixels) -> &mut [u8] {
    pixels.make_mut_bytes()
}

/// The bytes of a picture for writing (the engine copies decoded frames in here).
#[cfg(all(windows, not(feature = "slint")))]
pub(crate) fn pixel_bytes_mut(pixels: &mut Pixels) -> &mut [u8] {
    pixels.as_mut_slice()
}

/// An RGBA picture (4 bytes per pixel, rows packed with no padding).
///
/// `pixels` is `Pixels`: read it with `bytes()`, which works in both feature states.
#[derive(Clone)]
pub struct Rgba {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The pixels, row by row, four bytes each (red, green, blue, alpha). See `Pixels`.
    pub pixels: Pixels,
}

impl Rgba {
    /// A black picture of the given size.
    pub fn new(width: u32, height: u32) -> Rgba {
        Rgba {
            width,
            height,
            pixels: new_pixels(width, height),
        }
    }

    /// The bytes, row by row, four per pixel (red, green, blue, alpha).
    #[cfg(feature = "slint")]
    pub fn bytes(&self) -> &[u8] {
        self.pixels.as_bytes()
    }

    /// The bytes, row by row, four per pixel (red, green, blue, alpha).
    #[cfg(not(feature = "slint"))]
    pub fn bytes(&self) -> &[u8] {
        self.pixels.as_slice()
    }

    /// The picture as a Slint image, without copying the pixels (the `slint` feature).
    #[cfg(feature = "slint")]
    pub fn into_slint_image(self) -> slint::Image {
        slint::Image::from_rgba8(self.pixels)
    }
}

impl PartialEq for Rgba {
    fn eq(&self, other: &Rgba) -> bool {
        self.width == other.width && self.height == other.height && self.bytes() == other.bytes()
    }
}

impl fmt::Debug for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rgba")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// One video frame from the player: the picture and its time in the video.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// The picture, scaled to the player's output size.
    pub picture: Rgba,
    /// When the frame is shown, in milliseconds from the start of the video.
    pub pts_ms: u64,
}

/// Why a media call failed.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaError {
    /// Video is not available on this platform yet.
    Unsupported,
    /// The file has no video stream.
    NoVideo,
    /// The file could not be opened or read.
    Open(String),
    /// A video frame could not be decoded.
    Decode(String),
    /// The sound could not be decoded or played.
    Audio(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str("Video plays in the Windows app for now."),
            Self::NoVideo => f.write_str("This file has no video."),
            Self::Open(detail) => write!(f, "Could not open the video: {detail}"),
            Self::Decode(detail) => write!(f, "Could not decode the video: {detail}"),
            Self::Audio(detail) => write!(f, "Could not play the sound: {detail}"),
        }
    }
}

impl std::error::Error for MediaError {}

/// Settings for `Player::open`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerOptions {
    /// Largest width frames are scaled to (aspect kept inside it, never larger than the video).
    pub max_width: u32,
    /// Largest height frames are scaled to (aspect kept inside it, never larger than the video).
    pub max_height: u32,
    /// False: no sound and no audio device (tests, muted previews). The clock is then the wall clock.
    pub audio: bool,
}

#[cfg(windows)]
pub use engine::poster::poster;

/// Reads size, length, sound and container of a video file. A file with no video stream is `NoVideo`.
#[cfg(windows)]
pub fn probe(path: &Path) -> Result<MediaInfo, MediaError> {
    let details = engine::player::probe(path).map_err(MediaError::Open)?;
    if details.video.is_none() {
        return Err(MediaError::NoVideo);
    }
    Ok(MediaInfo {
        width: details.width,
        height: details.height,
        duration_ms: details.duration_ms,
        has_audio: !details.audio.is_empty(),
        mime: mime_for(path, &details.container),
    })
}

/// The MIME type from the container name FFmpeg gives and the file's extension (as rimelight-space always did).
#[cfg(windows)]
fn mime_for(path: &Path, container: &str) -> &'static str {
    let ext = engine::files::ext_of(path);
    match container {
        "mov" => {
            if ext == "mov" {
                "video/quicktime"
            } else {
                "video/mp4"
            }
        }
        "matroska" => {
            if ext == "webm" {
                "video/webm"
            } else {
                "video/x-matroska"
            }
        }
        "webm" => "video/webm",
        _ => "video/mp4",
    }
}

/// Video player with sound. Frames come from `take_frame`, which the UI calls from a timer.
///
/// Wraps `engine::player::Player`. The engine's volume is squared (a gentler curve), so `set_volume` takes the square
/// root here and the volume is linear as before.
#[cfg(windows)]
pub struct Player {
    inner: engine::player::Player,
}

#[cfg(windows)]
impl Player {
    /// Opens a video and starts its decode threads. Paused at 0. Without an audio device (or with `audio: false`)
    /// it plays silently on the wall clock.
    pub fn open(path: &Path, options: PlayerOptions) -> Result<Player, MediaError> {
        let mut details = engine::player::probe(path).map_err(MediaError::Open)?;
        if details.video.is_none() {
            return Err(MediaError::NoVideo);
        }
        if !options.audio {
            details.audio.clear();
        }
        let inner = engine::player::Player::open(
            path,
            &details,
            None,
            0,
            (options.max_width, options.max_height),
        )
        .map_err(MediaError::Open)?;
        Ok(Player { inner })
    }

    /// Starts playback. An ended video starts again from the beginning.
    pub fn play(&self) {
        self.inner.play();
    }

    /// Pauses playback.
    pub fn pause(&self) {
        self.inner.pause();
    }

    /// True while playing.
    pub fn is_playing(&self) -> bool {
        self.inner.is_playing()
    }

    /// Jumps to `ms` (clamped to the length). Keeps playing or paused as it was.
    pub fn seek(&self, ms: u64) {
        self.inner.seek(ms);
    }

    /// Sets the volume (0 to 1, clamped).
    pub fn set_volume(&self, volume: f32) {
        self.inner.set_volume(volume.clamp(0.0, 1.0).sqrt());
    }

    /// Mutes or unmutes the sound. The clock keeps running while muted.
    pub fn set_muted(&self, muted: bool) {
        self.inner.set_muted(muted);
    }

    /// Turns looping on or off. When on, the video starts again at its end.
    pub fn set_looping(&self, looping: bool) {
        self.inner.set_looping(looping);
    }

    /// New output size for the next frames (the decode thread rebuilds its scaler).
    pub fn set_output_size(&self, max_width: u32, max_height: u32) {
        self.inner.set_output_size(max_width, max_height);
    }

    /// Current position in milliseconds (never past the length).
    pub fn position_ms(&self) -> u64 {
        self.inner.position_ms()
    }

    /// Length in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.inner.duration_ms()
    }

    /// True when the file has sound AND it is being played through a device.
    pub fn has_audio(&self) -> bool {
        self.inner.has_audio()
    }

    /// True when playback reached the end and is not looping.
    pub fn ended(&self) -> bool {
        self.inner.ended()
    }

    /// The newest decoded frame whose time is at or before the clock. Older waiting frames are dropped. None when no
    /// new frame is due. The UI calls this from a timer (about every 5 ms).
    pub fn take_frame(&self) -> Option<VideoFrame> {
        self.inner.take_frame()
    }

    /// The last error from the decoder or the sound stream, if any.
    pub fn error(&self) -> Option<String> {
        self.inner.error()
    }
}
