//! SVG pictures through resvg, and the conversions between resvg's pixmaps and `Picture`.

use std::sync::{Arc, OnceLock};

use resvg::{tiny_skia, usvg};

use crate::picture::Picture;

/// SVG is drawn so its longer side is at least this many pixels (sharp when zoomed a little).
pub const SVG_MIN_SIDE: f32 = 2048.0;

/// The fonts SVG text uses: the system's on desktop and Android targets (none on wasm32), Segoe UI as sans-serif.
fn fonts() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        #[cfg(not(target_arch = "wasm32"))]
        db.load_system_fonts();
        db.set_sans_serif_family("Segoe UI");
        Arc::new(db)
    })
    .clone()
}

/// The options every SVG parse uses (the shared font database).
pub(crate) fn options() -> usvg::Options<'static> {
    usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    }
}

/// Draws SVG bytes so the longer side is at least `SVG_MIN_SIDE` pixels (never smaller than the drawing's own size).
pub fn load_svg(bytes: &[u8]) -> Result<Picture, String> {
    let tree = usvg::Tree::from_data(bytes, &options()).map_err(|e| e.to_string())?;
    let size = tree.size();
    let scale = (SVG_MIN_SIDE / size.width().max(size.height())).max(1.0);
    let w = (size.width() * scale).ceil().max(1.0) as u32;
    let h = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("the drawing is too large")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(from_pixmap(&pixmap))
}

/// SVG bytes drawn into a `size` x `size` square, scaled from the drawing's width (the app icon). None when it does not
/// parse.
pub fn render_svg_square(bytes: &[u8], size: u32) -> Option<Picture> {
    let tree = usvg::Tree::from_data(bytes, &options()).ok()?;
    let scale = size as f32 / tree.size().width();
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Some(from_pixmap(&pixmap))
}

/// A premultiplied pixmap as straight RGBA.
pub(crate) fn from_pixmap(p: &tiny_skia::Pixmap) -> Picture {
    let mut rgba = Vec::with_capacity(p.data().len());
    for px in p.pixels() {
        let c = px.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Picture {
        w: p.width(),
        h: p.height(),
        rgba,
    }
}

/// Straight RGBA as a premultiplied pixmap (for drawing onto).
pub(crate) fn to_pixmap(pic: &Picture) -> Option<tiny_skia::Pixmap> {
    let mut data = Vec::with_capacity(pic.rgba.len());
    for c in pic.rgba.as_chunks::<4>().0 {
        let a = u16::from(c[3]);
        let m = |v: u8| ((u16::from(v) * a + 127) / 255) as u8;
        data.extend_from_slice(&[m(c[0]), m(c[1]), m(c[2]), c[3]]);
    }
    tiny_skia::Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(pic.w, pic.h)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_render_has_the_requested_size() {
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='10' height='10' fill='#00f'/></svg>";
        let pic = render_svg_square(svg, 32).expect("renders");
        assert_eq!((pic.w, pic.h), (32, 32));
        assert!(render_svg_square(b"not svg", 32).is_none());
    }
}
