# rimelight_picture

Picture files without a UI toolkit, shared by Rimelight Media and Rimelight Space.

- **Decode** PNG, JPEG, WebP, GIF, BMP, ICO and SVG (always). HEIC, HEIF, HIF and AVIF with the `heif` feature (FFmpeg).
  Camera RAW with the `raw` feature (rawler). Pictures are straight RGBA, with EXIF orientation applied.
- **Thumbnails**: `Picture::thumbnail(max_side)` and `Picture::fit(w, h)`, Triangle filter, never upscaling.
- **Animation**: GIF and WebP frames with delays, under `AnimationLimits`.
- **Edit**: rotate, flip, crop, resize, canvas resize, rectangle fill, paint bucket, and markup burned in (`Mark`).
- **Save**: PNG, JPEG (quality 92), lossless WebP and BMP, by extension or into bytes.

Converting a `Picture` to a Slint image stays in the app.

`unsafe` code appears only in `src/heif.rs`, behind the `heif` feature.
