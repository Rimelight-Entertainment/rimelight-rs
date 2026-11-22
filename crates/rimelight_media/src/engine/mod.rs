//! The FFmpeg and cpal engine (Windows only): the player (tracks, subtitles text and picture, speed, sound output),
//! playback speed without a pitch change, trimming without re-encoding, and the poster frame the public API uses.
//!
//! Everything here reports errors as `String` or `MediaError`. The public API in the crate root wraps it.

pub mod bitmap_sub;
pub mod export;
pub mod files;
pub mod player;
pub mod poster;
pub mod stretch;
pub mod subs;
