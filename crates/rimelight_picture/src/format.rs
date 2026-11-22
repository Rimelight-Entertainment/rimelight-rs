//! Which reader a picture file goes to, by its extension (lower case compared).

use std::path::Path;

/// The reader a picture file goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// PNG, JPEG, WebP, GIF, BMP, ICO and anything else the `image` crate sniffs.
    Raster,
    /// SVG, drawn by resvg.
    Svg,
    /// HEIC, HEIF, HIF and AVIF (FFmpeg, with the `heif` feature).
    Heif,
    /// Camera RAW (rawler, with the `raw` feature).
    Raw,
}

/// HEIC, HEIF, HIF and AVIF.
pub const HEIF_EXTENSIONS: &[&str] = &["heic", "heif", "hif", "avif"];

/// Camera RAW formats.
pub const RAW_EXTENSIONS: &[&str] = &[
    "dng", "cr2", "cr3", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw",
    "3fr", "erf", "kdc", "mrw", "x3f", "iiq", "rwl",
];

/// The extension of a path, lower case ("" when there is none).
pub fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The reader for a path, by its extension.
pub fn format_of(path: &Path) -> Format {
    format_of_ext(&ext_of(path))
}

/// The reader for an extension without its dot. Case is ignored.
pub fn format_of_ext(ext: &str) -> Format {
    let ext = ext.to_ascii_lowercase();
    if ext == "svg" {
        Format::Svg
    } else if HEIF_EXTENSIONS.contains(&ext.as_str()) {
        Format::Heif
    } else if RAW_EXTENSIONS.contains(&ext.as_str()) {
        Format::Raw
    } else {
        Format::Raster
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats_by_extension() {
        assert_eq!(format_of(Path::new("a.PNG")), Format::Raster);
        assert_eq!(format_of(Path::new("photo.JPG")), Format::Raster);
        assert_eq!(format_of(Path::new("logo.svg")), Format::Svg);
        assert_eq!(format_of(Path::new("IMG_1.HEIC")), Format::Heif);
        assert_eq!(format_of(Path::new("clip.avif")), Format::Heif);
        assert_eq!(format_of(Path::new("shot.cr3")), Format::Raw);
        assert_eq!(format_of(Path::new("no_extension")), Format::Raster);
        assert_eq!(ext_of(Path::new("dir/Name.TIFF")), "tiff");
    }
}
