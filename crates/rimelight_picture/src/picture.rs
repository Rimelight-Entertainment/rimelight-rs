//! The picture type, decoding by path or bytes, thumbnails, animation frames and encoding.

use std::io::{Cursor, Seek, Write};
use std::path::Path;

use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{AnimationDecoder, ImageDecoder};

use crate::format::{self, Format};

/// An RGBA picture: rows packed, top to bottom, straight (not premultiplied) alpha.
#[derive(Clone, Debug)]
pub struct Picture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Picture {
    /// A picture from an `RgbaImage` (the pixels move, no copy).
    pub fn from_rgba_image(img: image::RgbaImage) -> Picture {
        Picture {
            w: img.width(),
            h: img.height(),
            rgba: img.into_raw(),
        }
    }

    /// A copy of the pixels as an `RgbaImage`, for the `image` crate's filters and encoders.
    pub fn to_rgba_image(&self) -> image::RgbaImage {
        image::RgbaImage::from_raw(self.w, self.h, self.rgba.clone())
            .unwrap_or_else(|| image::RgbaImage::new(1, 1))
    }

    /// Scales down, never up, so the picture fits in `max_w` x `max_h` with its aspect ratio kept. A limit of 0 means
    /// no limit on that side. Triangle filter.
    pub fn fit(&self, max_w: u32, max_h: u32) -> Picture {
        let fw = if max_w == 0 {
            1.0
        } else {
            f64::from(max_w) / f64::from(self.w.max(1))
        };
        let fh = if max_h == 0 {
            1.0
        } else {
            f64::from(max_h) / f64::from(self.h.max(1))
        };
        let scale = fw.min(fh).min(1.0);
        if scale >= 1.0 {
            return self.clone();
        }
        let w = (f64::from(self.w) * scale).round().max(1.0) as u32;
        let h = (f64::from(self.h) * scale).round().max(1.0) as u32;
        crate::edit::resample(self, w, h, FilterType::Triangle)
    }

    /// The picture with its longer side at most `max_side` (no upscaling). 0 keeps it as it is.
    pub fn thumbnail(&self, max_side: u32) -> Picture {
        self.fit(max_side, max_side)
    }
}

/// How much of an animation is kept.
#[derive(Clone, Copy, Debug)]
pub struct AnimationLimits {
    /// Most frames kept.
    pub max_frames: usize,
    /// Most decoded RGBA bytes kept.
    pub max_bytes: usize,
}

impl Default for AnimationLimits {
    /// No frame limit; 1.2 GB of decoded RGBA (Media's limit).
    fn default() -> Self {
        AnimationLimits {
            max_frames: usize::MAX,
            max_bytes: 1_200_000_000,
        }
    }
}

/// An animated GIF or WebP: every frame, full canvas, each with how long it shows (ms).
pub struct Animation {
    pub frames: Vec<(Picture, u32)>,
    /// True when frames were left out to stay under a limit.
    pub cut: bool,
}

/// Opens a picture file, upright (its EXIF orientation applied). SVG is drawn at its natural size with a minimum longer
/// side (see `svg::SVG_MIN_SIDE`). HEIF and RAW need the `heif` and `raw` features; without them this is an error.
pub fn load(path: &Path) -> Result<Picture, String> {
    match format::format_of(path) {
        Format::Svg => {
            let data = std::fs::read(path).map_err(|e| e.to_string())?;
            crate::svg::load_svg(&data)
        }
        Format::Heif => heif_load(path),
        Format::Raw => raw_load(path),
        Format::Raster => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            load_bytes(&bytes)
        }
    }
}

/// Decodes picture bytes (PNG, JPEG, WebP, GIF, BMP, ICO, or SVG text). The format is sniffed from the bytes, not the
/// name. Animated files give their first frame; use `load_animation_bytes` for every frame.
pub fn load_bytes(bytes: &[u8]) -> Result<Picture, String> {
    if looks_like_svg(bytes) {
        return crate::svg::load_svg(bytes);
    }
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    Ok(Picture::from_rgba_image(img.into_rgba8()))
}

/// SVG text is XML that starts with "<" (after blanks and an optional byte order mark). Raster formats are sniffed first.
fn looks_like_svg(bytes: &[u8]) -> bool {
    if image::guess_format(bytes).is_ok() {
        return false;
    }
    let body = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };
    body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<')
}

/// A picture file fitted inside `max_w` x `max_h` (see `Picture::fit`), upright. None when it does not open.
pub fn thumbnail_within(path: &Path, max_w: u32, max_h: u32) -> Option<Picture> {
    load(path).ok().map(|pic| pic.fit(max_w, max_h))
}

/// A picture file with its longer side at most `max_side`, upright. None when it does not open.
pub fn thumbnail(path: &Path, max_side: u32) -> Option<Picture> {
    thumbnail_within(path, max_side, max_side)
}

/// The frames of an animated GIF or WebP file. None for a still picture or any other format.
pub fn load_animation(path: &Path) -> Option<Animation> {
    load_animation_limited(path, &AnimationLimits::default())
}

/// The frames of an animated GIF or WebP file, within `limits`. None for a still picture or any other format.
pub fn load_animation_limited(path: &Path, limits: &AnimationLimits) -> Option<Animation> {
    let bytes = std::fs::read(path).ok()?;
    load_animation_bytes(&bytes, limits)
}

/// The frames of animated GIF or WebP bytes, within `limits`. None for a still picture (or one frame) or any other
/// format. A frame delay of 10 ms or less shows as 100 ms, as browsers do.
pub fn load_animation_bytes(bytes: &[u8], limits: &AnimationLimits) -> Option<Animation> {
    let frames: image::Frames<'_> = match image::guess_format(bytes).ok()? {
        image::ImageFormat::Gif => image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
            .ok()?
            .into_frames(),
        image::ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).ok()?;
            if !decoder.has_animation() {
                return None;
            }
            decoder.into_frames()
        }
        _ => return None,
    };
    let mut out = Vec::new();
    let mut total = 0usize;
    let mut cut = false;
    for frame in frames {
        let Ok(frame) = frame else { break };
        if out.len() >= limits.max_frames {
            cut = true;
            break;
        }
        let (num, den) = frame.delay().numer_denom_ms();
        let ms = num.checked_div(den).unwrap_or(0);
        let ms = if ms <= 10 { 100 } else { ms };
        let buf = frame.into_buffer();
        total += buf.as_raw().len();
        if total > limits.max_bytes {
            cut = true;
            break;
        }
        out.push((Picture::from_rgba_image(buf), ms));
    }
    (out.len() > 1).then_some(Animation { frames: out, cut })
}

/// Can this path's extension be written back? (PNG, JPEG, WebP and BMP.)
pub fn can_save(path: &Path) -> bool {
    matches!(
        format::ext_of(path).as_str(),
        "png" | "jpg" | "jpeg" | "jfif" | "webp" | "bmp"
    )
}

/// Saves by extension (see `write_encoded`).
pub fn save(pic: &Picture, path: &Path) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut out = std::io::BufWriter::new(file);
    write_encoded(pic, &format::ext_of(path), &mut out)?;
    out.flush().map_err(|e| e.to_string())
}

/// Encodes to bytes by extension (see `write_encoded`).
pub fn encode(pic: &Picture, ext: &str) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    write_encoded(pic, ext, &mut out)?;
    Ok(out.into_inner())
}

/// Lossless WebP bytes (Space's thumbnails).
pub fn encode_webp_lossless(pic: &Picture) -> Result<Vec<u8>, String> {
    encode(pic, "webp")
}

/// Writes the picture in the format `ext` names: "jpg", "jpeg" or "jfif" (JPEG, quality 92), "webp" (lossless),
/// "bmp", or PNG for anything else.
pub fn write_encoded(
    pic: &Picture,
    ext: &str,
    out: &mut (impl Write + Seek),
) -> Result<(), String> {
    let img = pic.to_rgba_image();
    let result = match ext {
        "jpg" | "jpeg" | "jfif" => {
            let rgb = image::DynamicImage::ImageRgba8(img).into_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(out, 92).encode_image(&rgb)
        }
        "webp" => image::codecs::webp::WebPEncoder::new_lossless(out).encode(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        ),
        "bmp" => image::DynamicImage::ImageRgba8(img).write_to(out, image::ImageFormat::Bmp),
        _ => image::DynamicImage::ImageRgba8(img).write_to(out, image::ImageFormat::Png),
    };
    result.map_err(|e| e.to_string())
}

#[cfg(all(feature = "heif", not(target_arch = "wasm32")))]
fn heif_load(path: &Path) -> Result<Picture, String> {
    crate::heif::load(path)
}

#[cfg(not(all(feature = "heif", not(target_arch = "wasm32"))))]
fn heif_load(_path: &Path) -> Result<Picture, String> {
    Err("HEIC, HEIF and AVIF pictures need the `heif` feature of rimelight_picture".to_string())
}

#[cfg(feature = "raw")]
fn raw_load(path: &Path) -> Result<Picture, String> {
    crate::raw::load(path)
}

#[cfg(not(feature = "raw"))]
fn raw_load(_path: &Path) -> Result<Picture, String> {
    Err("camera RAW pictures need the `raw` feature of rimelight_picture".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(w: u32, h: u32) -> Picture {
        Picture {
            w,
            h,
            rgba: vec![128; (w * h * 4) as usize],
        }
    }

    fn checker(w: u32, h: u32) -> Picture {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&[(x * 10) as u8, (y * 10) as u8, 0, 255]);
            }
        }
        Picture { w, h, rgba }
    }

    #[test]
    fn thumbnail_keeps_the_aspect_and_never_upscales() {
        let big = blank(2000, 1000);
        let t = big.thumbnail(256);
        assert_eq!((t.w, t.h), (256, 128));
        let small = blank(10, 5);
        let t = small.thumbnail(256);
        assert_eq!((t.w, t.h), (10, 5));
        let tall = blank(1, 1000);
        let t = tall.thumbnail(100);
        assert_eq!((t.w, t.h), (1, 100));
        let wide = blank(200, 100);
        assert_eq!((wide.fit(0, 50).w, wide.fit(0, 50).h), (100, 50));
        assert_eq!((wide.fit(50, 0).w, wide.fit(50, 0).h), (50, 25));
    }

    #[test]
    fn thumbnail_webp_has_the_thumbnail_size() {
        let webp = encode_webp_lossless(&blank(2000, 1000).thumbnail(256)).expect("encodes");
        let back = load_bytes(&webp).expect("decodes");
        assert_eq!((back.w, back.h), (256, 128));
    }

    #[test]
    fn png_round_trip_is_exact() {
        let pic = checker(3, 2);
        let png = encode(&pic, "png").expect("encodes");
        let back = load_bytes(&png).expect("decodes");
        assert_eq!((back.w, back.h), (3, 2));
        assert_eq!(back.rgba, pic.rgba);
    }

    #[test]
    fn webp_lossless_round_trip_is_exact() {
        let pic = checker(5, 4);
        let webp = encode_webp_lossless(&pic).expect("encodes");
        let back = load_bytes(&webp).expect("decodes");
        assert_eq!(back.rgba, pic.rgba);
    }

    #[test]
    fn svg_text_is_sniffed_and_drawn_at_the_minimum_side() {
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='16' height='32'><rect width='16' height='32' fill='red'/></svg>";
        let pic = load_bytes(svg).expect("draws");
        assert_eq!((pic.w, pic.h), (1024, 2048));
    }

    #[test]
    fn gif_animation_frames_and_delays() {
        let frame = |c: [u8; 4]| {
            image::Frame::from_parts(
                image::RgbaImage::from_pixel(4, 4, image::Rgba(c)),
                0,
                0,
                image::Delay::from_numer_denom_ms(50, 1),
            )
        };
        let mut gif = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut gif)
            .encode_frames(vec![frame([255, 0, 0, 255]), frame([0, 0, 255, 255])])
            .expect("encodes");
        let anim = load_animation_bytes(&gif, &AnimationLimits::default()).expect("two frames");
        assert_eq!(anim.frames.len(), 2);
        assert_eq!(anim.frames[0].1, 50);
        assert!(!anim.cut);
        let limited = AnimationLimits {
            max_frames: 1,
            max_bytes: usize::MAX,
        };
        assert!(load_animation_bytes(&gif, &limited).is_none());

        let mut still = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut still)
            .encode_frames(vec![frame([0, 255, 0, 255])])
            .expect("encodes");
        assert!(load_animation_bytes(&still, &AnimationLimits::default()).is_none());
    }
}
