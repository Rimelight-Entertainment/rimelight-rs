//! Markup marks (pen, highlighter, arrow, box, text, picture, ellipse, line, eraser) burned into a picture.

use std::sync::Arc;

use resvg::{tiny_skia, usvg};

use crate::picture::Picture;
use crate::svg::{from_pixmap, options, to_pixmap};

/// The font the text marks use (SVG font-family).
const TEXT_FONT: &str = "Segoe UI";

/// One markup mark in picture pixels.
#[derive(Clone, Debug)]
pub struct Mark {
    /// 0 pen, 1 highlighter, 2 arrow, 3 box, 4 text, 5 picture, 6 ellipse, 7 line, 8 eraser.
    pub kind: i32,
    pub rgba: [u8; 4],
    pub width: f32,
    pub points: Vec<(f32, f32)>,
    pub text: String,
    /// A box or an ellipse filled with its colour.
    pub filled: bool,
    /// A picture mark: its pixels, drawn into the rectangle from points[0] to points[1].
    pub image: Option<Arc<Picture>>,
}

impl Mark {
    /// The box round the mark: x, y, width, height (picture pixels).
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        let Some(&(x0, y0)) = self.points.first() else {
            return (0.0, 0.0, 0.0, 0.0);
        };
        if self.kind == 4 {
            return (
                x0,
                y0,
                self.font_size() * 0.6 * self.text.chars().count().max(1) as f32,
                self.font_size() * 1.3,
            );
        }
        let (mut x1, mut y1, mut x2, mut y2) = (x0, y0, x0, y0);
        for &(x, y) in &self.points {
            x1 = x1.min(x);
            y1 = y1.min(y);
            x2 = x2.max(x);
            y2 = y2.max(y);
        }
        (x1, y1, x2 - x1, y2 - y1)
    }

    /// The path the mark draws, in the SVG path syntax.
    pub fn commands(&self) -> String {
        let p = &self.points;
        if p.is_empty() {
            return String::new();
        }
        let (x0, y0) = p[0];
        let (x1, y1) = *p.last().unwrap_or(&p[0]);
        match self.kind {
            5 => String::new(),
            7 => format!("M {x0:.1} {y0:.1} L {x1:.1} {y1:.1}"),
            6 => {
                // An ellipse in four cubic curves inside the box from (x0, y0) to (x1, y1).
                const K: f32 = 0.5523;
                let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                let (rx, ry) = ((x1 - x0).abs() / 2.0, (y1 - y0).abs() / 2.0);
                let (kx, ky) = (rx * K, ry * K);
                format!(
                    "M {cx:.1} {:.1} C {:.1} {:.1} {:.1} {:.1} {:.1} {cy:.1} C {:.1} {:.1} {:.1} {:.1} {cx:.1} {:.1} C {:.1} {:.1} {:.1} {:.1} {:.1} {cy:.1} C {:.1} {:.1} {:.1} {:.1} {cx:.1} {:.1} Z",
                    cy - ry,
                    cx + kx,
                    cy - ry,
                    cx + rx,
                    cy - ky,
                    cx + rx,
                    cx + rx,
                    cy + ky,
                    cx + kx,
                    cy + ry,
                    cy + ry,
                    cx - kx,
                    cy + ry,
                    cx - rx,
                    cy + ky,
                    cx - rx,
                    cx - rx,
                    cy - ky,
                    cx - kx,
                    cy - ry,
                    cy - ry,
                )
            }
            2 => {
                let ang = (y1 - y0).atan2(x1 - x0);
                let head = (self.width * 4.0).max(14.0);
                let a1 = ang + 2.6;
                let a2 = ang - 2.6;
                format!(
                    "M {x0:.1} {y0:.1} L {x1:.1} {y1:.1} M {:.1} {:.1} L {x1:.1} {y1:.1} L {:.1} {:.1}",
                    x1 + head * a1.cos(),
                    y1 + head * a1.sin(),
                    x1 + head * a2.cos(),
                    y1 + head * a2.sin()
                )
            }
            3 => {
                format!("M {x0:.1} {y0:.1} L {x1:.1} {y0:.1} L {x1:.1} {y1:.1} L {x0:.1} {y1:.1} Z")
            }
            _ => {
                let mut s = format!("M {x0:.1} {y0:.1}");
                if p.len() == 1 {
                    s.push_str(&format!(" L {:.1} {y0:.1}", x0 + 0.1));
                }
                for (x, y) in &p[1..] {
                    s.push_str(&format!(" L {x:.1} {y:.1}"));
                }
                s
            }
        }
    }

    /// Stroke width as drawn (the highlighter is wide).
    pub fn stroke_width(&self) -> f32 {
        if self.kind == 1 {
            self.width * 4.0
        } else {
            self.width
        }
    }

    /// Text size in picture pixels.
    pub fn font_size(&self) -> f32 {
        (self.width * 4.0).max(12.0)
    }

    /// Rough distance from (x, y) to the mark, for the eraser, the move tool and the picture marks (0 inside those).
    pub fn distance(&self, x: f32, y: f32) -> f32 {
        if self.points.is_empty() {
            return f32::MAX;
        }
        if self.kind == 4 || self.kind == 5 {
            let (tx, ty, w, h) = self.bounds();
            let dx = (tx - x).max(x - (tx + w)).max(0.0);
            let dy = (ty - y).max(y - (ty + h)).max(0.0);
            return dx.hypot(dy);
        }
        // The real ellipse: |(dx/rx)^2 + (dy/ry)^2 - 1| is the normalised distance; dividing by its gradient gives
        // pixels. An outline counts within half its stroke; a filled one counts inside.
        if self.kind == 6 && self.points.len() >= 2 {
            let (x0, y0) = self.points[0];
            let (x1, y1) = *self.points.last().unwrap_or(&self.points[0]);
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let (rx, ry) = ((x1 - x0).abs() / 2.0, (y1 - y0).abs() / 2.0);
            if rx > 0.0 && ry > 0.0 {
                let (dx, dy) = (x - cx, y - cy);
                let k = (dx / rx).powi(2) + (dy / ry).powi(2);
                if self.filled && k <= 1.0 {
                    return 0.0;
                }
                let grad = 2.0 * ((dx / (rx * rx)).powi(2) + (dy / (ry * ry)).powi(2)).sqrt();
                let pixels = if grad > 1e-6 {
                    (k - 1.0).abs() / grad
                } else {
                    rx.min(ry)
                };
                return if self.filled {
                    pixels
                } else {
                    (pixels - self.stroke_width() / 2.0).max(0.0)
                };
            }
        }
        // A line is its segment (its two points); a box is its outline.
        let pts: Vec<(f32, f32)> = if (self.kind == 3 || self.kind == 6) && self.points.len() >= 2 {
            let (x0, y0) = self.points[0];
            let (x1, y1) = *self.points.last().unwrap_or(&self.points[0]);
            vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]
        } else if self.kind == 2 && self.points.len() >= 2 {
            vec![
                self.points[0],
                *self.points.last().unwrap_or(&self.points[0]),
            ]
        } else {
            self.points.clone()
        };
        if pts.len() == 1 {
            return (pts[0].0 - x).hypot(pts[0].1 - y);
        }
        pts.windows(2)
            .map(|s| seg_dist((x, y), s[0], s[1]))
            .fold(f32::MAX, f32::min)
    }
}

fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a.0 + t * dx - p.0).hypot(a.1 + t * dy - p.1)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Draws the marks into the picture, in order. Vector marks are drawn through SVG onto a marks layer; a picture mark is
/// composited in place. An eraser stroke takes markup off the layer only, so a photo keeps its pixels (the eraser clears
/// the layer). On a canvas (`on_canvas`) the layer starts as the picture and the eraser paints its background colour.
pub fn burn_marks(pic: &Picture, marks: &[Mark], on_canvas: bool) -> Result<Picture, String> {
    if marks.is_empty() {
        return Ok(pic.clone());
    }
    let mut out = pic.clone();
    let mut layer = marks_layer(&out, on_canvas)?;
    let mut run: Vec<&Mark> = Vec::new();
    for m in marks {
        match m.kind {
            5 => {
                draw_run(&mut layer, &run)?;
                run.clear();
                out = settle(&out, &layer, on_canvas);
                composite(&mut out, m);
                layer = marks_layer(&out, on_canvas)?;
            }
            8 => {
                draw_run(&mut layer, &run)?;
                run.clear();
                erase(&mut layer, m, on_canvas);
            }
            _ => run.push(m),
        }
    }
    draw_run(&mut layer, &run)?;
    Ok(settle(&out, &layer, on_canvas))
}

/// The layer the marks are drawn on: the picture itself on a canvas, a clear layer on a photo.
fn marks_layer(pic: &Picture, on_canvas: bool) -> Result<tiny_skia::Pixmap, String> {
    if on_canvas {
        to_pixmap(pic).ok_or_else(|| "the picture is empty".to_string())
    } else {
        tiny_skia::Pixmap::new(pic.w, pic.h).ok_or_else(|| "the drawing is too large".to_string())
    }
}

/// The picture with the layer on it (a canvas is the layer; a photo gets the layer's marks laid over it).
fn settle(out: &Picture, layer: &tiny_skia::Pixmap, on_canvas: bool) -> Picture {
    if on_canvas {
        return from_pixmap(layer);
    }
    let Some(mut base) = to_pixmap(out) else {
        return out.clone();
    };
    base.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );
    from_pixmap(&base)
}

/// The vector marks (no picture or eraser marks) drawn onto the layer.
fn draw_run(layer: &mut tiny_skia::Pixmap, marks: &[&Mark]) -> Result<(), String> {
    if marks.is_empty() {
        return Ok(());
    }
    let (w, h) = (layer.width(), layer.height());
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">"
    );
    for m in marks {
        let [r, g, b, a] = m.rgba;
        let opacity = f32::from(a) / 255.0;
        if m.kind == 4 {
            let (x, y) = m.points.first().copied().unwrap_or_default();
            let size = m.font_size();
            svg.push_str(&format!(
                "<text x=\"{x}\" y=\"{}\" font-family=\"{TEXT_FONT}\" font-weight=\"600\" font-size=\"{size}\" fill=\"#{r:02x}{g:02x}{b:02x}\" fill-opacity=\"{opacity}\">{}</text>",
                y + size * 0.92,
                xml_escape(&m.text)
            ));
        } else {
            let fill = if m.filled {
                format!("#{r:02x}{g:02x}{b:02x}\" fill-opacity=\"{opacity}")
            } else {
                "none".to_string()
            };
            svg.push_str(&format!(
                "<path d=\"{}\" fill=\"{fill}\" stroke=\"#{r:02x}{g:02x}{b:02x}\" stroke-opacity=\"{opacity}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                m.commands(),
                m.stroke_width()
            ));
        }
    }
    svg.push_str("</svg>");
    let tree = usvg::Tree::from_str(&svg, &options()).map_err(|e| e.to_string())?;
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut layer.as_mut());
    Ok(())
}

/// A pixel eraser stroke on the layer: a photo's layer clears (Clear), a canvas paints its background (Source).
fn erase(layer: &mut tiny_skia::Pixmap, m: &Mark, on_canvas: bool) {
    let Some(&(x0, y0)) = m.points.first() else {
        return;
    };
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(x0, y0);
    if m.points.len() == 1 {
        pb.line_to(x0 + 0.1, y0);
    }
    for &(x, y) in m.points.iter().skip(1) {
        pb.line_to(x, y);
    }
    let Some(path) = pb.finish() else { return };
    let mut paint = tiny_skia::Paint {
        anti_alias: true,
        ..Default::default()
    };
    let [r, g, b, a] = m.rgba;
    paint.set_color_rgba8(r, g, b, a);
    paint.blend_mode = if on_canvas {
        tiny_skia::BlendMode::Source
    } else {
        tiny_skia::BlendMode::Clear
    };
    let stroke = tiny_skia::Stroke {
        width: m.width.max(1.0),
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };
    layer.stroke_path(
        &path,
        &paint,
        &stroke,
        tiny_skia::Transform::identity(),
        None,
    );
}

/// Puts a picture mark's pixels over the picture, scaled to its rectangle (alpha blended, straight alpha).
fn composite(out: &mut Picture, m: &Mark) {
    let Some(src) = m.image.as_deref() else {
        return;
    };
    if m.points.len() < 2 {
        return;
    }
    let (x0, y0) = m.points[0];
    let (x1, y1) = m.points[m.points.len() - 1];
    let (left, top) = (x0.min(x1).round() as i64, y0.min(y1).round() as i64);
    let w = ((x1 - x0).abs().round() as u32).max(1);
    let h = ((y1 - y0).abs().round() as u32).max(1);
    let scaled = if w == src.w && h == src.h {
        src.clone()
    } else {
        crate::edit::resize(src, w, h)
    };
    for sy in 0..h {
        let dy = top + i64::from(sy);
        if dy < 0 || dy >= i64::from(out.h) {
            continue;
        }
        for sx in 0..w {
            let dx = left + i64::from(sx);
            if dx < 0 || dx >= i64::from(out.w) {
                continue;
            }
            let si = ((sy * w + sx) * 4) as usize;
            let di = ((dy as u32 * out.w + dx as u32) * 4) as usize;
            let a = u32::from(scaled.rgba[si + 3]);
            if a == 0 {
                continue;
            }
            let inv = 255 - a;
            for c in 0..3 {
                out.rgba[di + c] =
                    ((u32::from(scaled.rgba[si + c]) * a + u32::from(out.rgba[di + c]) * inv + 127)
                        / 255) as u8;
            }
            out.rgba[di + 3] = (a + u32::from(out.rgba[di + 3]) * inv / 255).min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_burn_in() {
        let p = Picture {
            w: 40,
            h: 40,
            rgba: vec![255; 40 * 40 * 4],
        };
        let m = Mark {
            kind: 3,
            rgba: [255, 0, 0, 255],
            width: 4.0,
            points: vec![(5.0, 5.0), (35.0, 35.0)],
            text: String::new(),
            filled: false,
            image: None,
        };
        let out = burn_marks(&p, &[m], false).unwrap();
        let px = &out.rgba[((5 * 40 + 20) * 4) as usize..][..4];
        assert!(px[0] > 200 && px[1] < 80, "the box edge is red: {px:?}");
    }
}
