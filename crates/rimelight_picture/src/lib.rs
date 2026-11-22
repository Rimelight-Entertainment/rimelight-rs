//! Picture files without a UI toolkit: decode to straight RGBA, thumbnails, animation frames, pixel edits, markup burned
//! into a picture, and saving. Conversions to a Slint image stay in the apps.
//!
//! Formats: PNG, JPEG, WebP, GIF, BMP, ICO and SVG always; HEIC, HEIF, HIF and AVIF with the `heif` feature (FFmpeg);
//! camera RAW with the `raw` feature (rawler).

mod edit;
mod format;
mod markup;
mod picture;
mod svg;

#[cfg(feature = "raw")]
mod raw;

#[cfg(all(feature = "heif", not(target_arch = "wasm32")))]
mod heif;

pub use edit::{canvas_fit, crop, crop_px, fill_rect, flip, flood_fill, resize, rotate_cw};
pub use format::{Format, HEIF_EXTENSIONS, RAW_EXTENSIONS, ext_of, format_of, format_of_ext};
pub use markup::{Mark, burn_marks};
pub use picture::{
    Animation, AnimationLimits, Picture, can_save, encode, encode_webp_lossless, load,
    load_animation, load_animation_bytes, load_animation_limited, load_bytes, save, thumbnail,
    thumbnail_within, write_encoded,
};
pub use svg::{SVG_MIN_SIDE, load_svg, render_svg_square};
