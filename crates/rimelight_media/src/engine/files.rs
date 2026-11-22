//! File-name helpers the engine needs (the app's own file rules stay in the app).

use std::path::Path;

/// Subtitle file extensions read from disk.
pub const SUBTITLE_EXTS: &[&str] = &["srt", "vtt", "ass", "ssa"];

/// The lower-case extension without the dot ("" when there is none).
pub fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The file name as text ("" when it is not valid text).
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string()
}
