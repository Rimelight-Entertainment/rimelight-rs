//! Picture subtitles (Blu-ray PGS, DVD, DVB): FFmpeg gives each as palette indices plus a palette, which ffmpeg-next
//! doesn't expose for FFmpeg 5 and later. This audited module reads them from the raw rectangle and returns RGBA.

use ffmpeg_next as ff;

/// One subtitle picture, placed in the subtitle canvas (usually the video's size).
#[derive(Debug, Clone)]
pub struct SubPicture {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// Reads one bitmap subtitle rectangle into RGBA. The only unsafe code in the crate.
#[allow(unsafe_code)]
pub fn read(rect: &ff::subtitle::Bitmap) -> Option<SubPicture> {
    // SAFETY: the rectangle comes from a subtitle FFmpeg just decoded and is alive while `rect` is borrowed. For bitmap
    // rectangles FFmpeg fills data[0] with h rows of w palette indices (linesize[0] bytes apart) and data[1] with
    // nb_colors 32-bit 0xAARRGGBB colours; both are checked for null and every index is bounds-checked.
    unsafe {
        let r = &*rect.as_ptr();
        let (w, h) = (r.w, r.h);
        if w <= 0 || h <= 0 || r.data[0].is_null() || r.data[1].is_null() || r.nb_colors <= 0 {
            return None;
        }
        let colours = std::slice::from_raw_parts(r.data[1].cast::<u32>(), r.nb_colors as usize);
        let stride = r.linesize[0] as usize;
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let row = std::slice::from_raw_parts(r.data[0].add(y * stride), w as usize);
            for &i in row {
                let c = colours.get(usize::from(i)).copied().unwrap_or(0);
                rgba.extend_from_slice(&[
                    (c >> 16) as u8,
                    (c >> 8) as u8,
                    c as u8,
                    (c >> 24) as u8,
                ]);
            }
        }
        Some(SubPicture {
            x: r.x,
            y: r.y,
            w: w as u32,
            h: h as u32,
            rgba,
        })
    }
}
