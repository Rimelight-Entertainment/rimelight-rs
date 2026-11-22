//! Stand-in backend for platforms without FFmpeg and cpal. Every call reports `MediaError::Unsupported`.

use std::path::Path;

use crate::{MediaError, MediaInfo, PlayerOptions, Rgba, VideoFrame};

/// Reads size, length, sound and container of a video file. Always `Unsupported` on this platform.
pub fn probe(_path: &Path) -> Result<MediaInfo, MediaError> {
    Err(MediaError::Unsupported)
}

/// One frame at `fraction` of the video. Always `Unsupported` on this platform.
pub fn poster(_path: &Path, _fraction: f64, _max_side: u32) -> Result<Rgba, MediaError> {
    Err(MediaError::Unsupported)
}

/// Video player stand-in. It cannot be built: `open` always fails.
pub struct Player {
    never: std::convert::Infallible,
}

impl Player {
    /// Always `Unsupported` on this platform.
    pub fn open(_path: &Path, _options: PlayerOptions) -> Result<Player, MediaError> {
        Err(MediaError::Unsupported)
    }

    /// Starts playback.
    pub fn play(&self) {
        match self.never {}
    }

    /// Pauses playback.
    pub fn pause(&self) {
        match self.never {}
    }

    /// True while playing.
    pub fn is_playing(&self) -> bool {
        match self.never {}
    }

    /// Jumps to `ms`.
    pub fn seek(&self, _ms: u64) {
        match self.never {}
    }

    /// Sets the volume (0 to 1).
    pub fn set_volume(&self, _volume: f32) {
        match self.never {}
    }

    /// Mutes or unmutes the sound.
    pub fn set_muted(&self, _muted: bool) {
        match self.never {}
    }

    /// Turns looping on or off.
    pub fn set_looping(&self, _looping: bool) {
        match self.never {}
    }

    /// Sets the output size for the next frames.
    pub fn set_output_size(&self, _max_width: u32, _max_height: u32) {
        match self.never {}
    }

    /// Current position in milliseconds.
    pub fn position_ms(&self) -> u64 {
        match self.never {}
    }

    /// Length in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        match self.never {}
    }

    /// True when the file has sound and it is played through a device.
    pub fn has_audio(&self) -> bool {
        match self.never {}
    }

    /// True when playback reached the end and is not looping.
    pub fn ended(&self) -> bool {
        match self.never {}
    }

    /// The newest frame that is due, if any.
    pub fn take_frame(&self) -> Option<VideoFrame> {
        match self.never {}
    }

    /// The last error from the decoder or the sound, if any.
    pub fn error(&self) -> Option<String> {
        match self.never {}
    }
}
