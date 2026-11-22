//! Pixel edits: rotate, flip, crop, resize, canvas resize, rectangle fill and the paint bucket. Each returns a new picture.

use image::imageops::FilterType;

use crate::picture::Picture;

/// Rotated a quarter turn clockwise.
pub fn rotate_cw(pic: &Picture) -> Picture {
    Picture::from_rgba_image(image::imageops::rotate90(&pic.to_rgba_image()))
}

/// Mirrored left to right (`left_right`) or top to bottom.
pub fn flip(pic: &Picture, left_right: bool) -> Picture {
    let img = pic.to_rgba_image();
    Picture::from_rgba_image(if left_right {
        image::imageops::flip_horizontal(&img)
    } else {
        image::imageops::flip_vertical(&img)
    })
}

/// Crops to a box given as fractions of the picture (x, y, width, height in 0..=1).
pub fn crop(pic: &Picture, fx: f32, fy: f32, fw: f32, fh: f32) -> Picture {
    let x = ((fx.clamp(0.0, 1.0) * pic.w as f32).round() as u32).min(pic.w.saturating_sub(1));
    let y = ((fy.clamp(0.0, 1.0) * pic.h as f32).round() as u32).min(pic.h.saturating_sub(1));
    let w = ((fw * pic.w as f32).round() as u32).clamp(1, pic.w - x);
    let h = ((fh * pic.h as f32).round() as u32).clamp(1, pic.h - y);
    crop_px(pic, x, y, w, h)
}

/// Crops to a box in picture pixels.
pub fn crop_px(pic: &Picture, x: u32, y: u32, w: u32, h: u32) -> Picture {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let start = ((row * pic.w + x) * 4) as usize;
        rgba.extend_from_slice(&pic.rgba[start..start + (w * 4) as usize]);
    }
    Picture { w, h, rgba }
}

/// Resized to w x h: Lanczos3 when shrinking, Catmull-Rom when growing.
pub fn resize(pic: &Picture, w: u32, h: u32) -> Picture {
    let filter = if w < pic.w {
        FilterType::Lanczos3
    } else {
        FilterType::CatmullRom
    };
    resample(pic, w, h, filter)
}

/// Resized to w x h with the given filter (zero sizes become 1).
pub(crate) fn resample(pic: &Picture, w: u32, h: u32, filter: FilterType) -> Picture {
    Picture::from_rgba_image(image::imageops::resize(
        &pic.to_rgba_image(),
        w.max(1),
        h.max(1),
        filter,
    ))
}

/// A canvas resized to w x h: the pixels stay at the top-left (the picture is cropped, or the new area is left), and
/// the new area is `bg`.
pub fn canvas_fit(pic: &Picture, w: u32, h: u32, bg: [u8; 4]) -> Picture {
    let mut rgba = bg.repeat(w as usize * h as usize);
    let (cw, ch) = (pic.w.min(w) as usize, pic.h.min(h) as usize);
    for y in 0..ch {
        let src = y * pic.w as usize * 4;
        let dst = y * w as usize * 4;
        rgba[dst..dst + cw * 4].copy_from_slice(&pic.rgba[src..src + cw * 4]);
    }
    Picture { w, h, rgba }
}

/// The picture with a rectangle (picture pixels) filled with one colour.
pub fn fill_rect(pic: &Picture, x: u32, y: u32, w: u32, h: u32, rgba: [u8; 4]) -> Picture {
    let mut out = pic.clone();
    for row in y..(y + h).min(pic.h) {
        for col in x..(x + w).min(pic.w) {
            let i = ((row * pic.w + col) * 4) as usize;
            out.rgba[i..i + 4].copy_from_slice(&rgba);
        }
    }
    out
}

/// Paint bucket: fills the area around (x, y) whose colours are within `tolerance` of the seed pixel (scanline fill).
/// A translucent fill colour is blended over the pixels.
pub fn flood_fill(pic: &Picture, x: u32, y: u32, fill: [u8; 4], tolerance: u8) -> Picture {
    let mut out = pic.clone();
    if x >= pic.w || y >= pic.h {
        return out;
    }
    let (w, h) = (pic.w as usize, pic.h as usize);
    let at = |px: usize, py: usize| (py * w + px) * 4;
    let seed = at(x as usize, y as usize);
    let target = [
        pic.rgba[seed],
        pic.rgba[seed + 1],
        pic.rgba[seed + 2],
        pic.rgba[seed + 3],
    ];
    let tol = i32::from(tolerance);
    let similar =
        |i: usize| (0..4).all(|c| (i32::from(pic.rgba[i + c]) - i32::from(target[c])).abs() <= tol);
    let mut done = vec![false; w * h];
    let mut stack = vec![(x as usize, y as usize)];
    let a = u32::from(fill[3]);
    while let Some((sx, sy)) = stack.pop() {
        if done[sy * w + sx] || !similar(at(sx, sy)) {
            continue;
        }
        // Widen the run to the left and right of the seed.
        let mut l = sx;
        while l > 0 && !done[sy * w + l - 1] && similar(at(l - 1, sy)) {
            l -= 1;
        }
        let mut r = sx;
        while r + 1 < w && !done[sy * w + r + 1] && similar(at(r + 1, sy)) {
            r += 1;
        }
        for cx in l..=r {
            done[sy * w + cx] = true;
            let i = at(cx, sy);
            for c in 0..3 {
                out.rgba[i + c] =
                    ((u32::from(fill[c]) * a + u32::from(pic.rgba[i + c]) * (255 - a) + 127) / 255)
                        as u8;
            }
            out.rgba[i + 3] = pic.rgba[i + 3].max(fill[3]);
        }
        // Seeds on the rows above and below: one per run of matching pixels.
        for ny in [sy.wrapping_sub(1), sy + 1] {
            if ny >= h {
                continue;
            }
            let mut cx = l;
            while cx <= r {
                if !done[ny * w + cx] && similar(at(cx, ny)) {
                    stack.push((cx, ny));
                    while cx <= r && !done[ny * w + cx] && similar(at(cx, ny)) {
                        cx += 1;
                    }
                } else {
                    cx += 1;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn rotate_crop_resize() {
        let p = checker(4, 2);
        let r = rotate_cw(&p);
        assert_eq!((r.w, r.h), (2, 4));
        let c = crop(&p, 0.5, 0.0, 0.5, 1.0);
        assert_eq!((c.w, c.h), (2, 2));
        assert_eq!(c.rgba[0], 20);
        let s = resize(&p, 8, 4);
        assert_eq!((s.w, s.h), (8, 4));
    }

    #[test]
    fn canvas_fit_keeps_the_top_left_and_fills_the_rest() {
        let p = checker(2, 2);
        let c = canvas_fit(&p, 3, 1, [9, 9, 9, 255]);
        assert_eq!((c.w, c.h), (3, 1));
        assert_eq!(&c.rgba[0..4], &p.rgba[0..4]);
        assert_eq!(&c.rgba[8..12], &[9, 9, 9, 255]);
    }
}
