//! Text geometry with parley. `TextInput` does not expose glyph positions, so the editor lays the same text out with the
//! system fonts at the same wrap width and reads positions from there (design section 3). It serves three jobs: the
//! remote carets ([`caret_rect`]), which visual line the caret is on for Up and Down ([`line_edges`]), and the byte
//! offset nearest a horizontal position on a line ([`offset_at`]). Nothing here names a Slint type.
//!
//! The parley contexts are per thread (they are expensive to build), so repeated calls reuse them.

use std::cell::RefCell;

use parley::layout::{Alignment, AlignmentOptions};
use parley::{Affinity, Cursor, FontContext, Layout, LayoutContext, StyleProperty};

/// Width of a remote caret, in logical pixels, as the rectangle is drawn.
pub const CARET_WIDTH: f32 = 2.0;

type Lay = Layout<[u8; 4]>;

thread_local! {
    static CONTEXTS: RefCell<(FontContext, LayoutContext<[u8; 4]>)> =
        RefCell::new((FontContext::default(), LayoutContext::new()));
}

/// `text` laid out at `font_size` and wrapped to `width`.
fn laid_out(text: &str, font_size: f32, width: f32) -> Lay {
    // UNSURE: parley 0.x constructors and methods below are written from the crate's documented API and were not
    // compiled (no build was run). FontContext::default, LayoutContext::new, ranged_builder(fcx, text, scale, quantize),
    // Layout::break_all_lines, Layout::align, Cursor::from_byte_index, Cursor::geometry, Cursor::from_point, Cursor::index,
    // Layout::len, Layout::get and Line::metrics are the ones to confirm first.
    CONTEXTS.with(|cell| {
        let mut contexts = cell.borrow_mut();
        let (font_cx, layout_cx) = &mut *contexts;
        let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(font_size));
        let mut layout: Lay = builder.build(text);
        layout.break_all_lines(Some(width));
        layout.align(Alignment::Start, AlignmentOptions::default());
        layout
    })
}

/// `byte_offset` moved back to a character boundary inside `text`.
fn boundary(text: &str, byte_offset: usize) -> usize {
    let mut offset = byte_offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The caret rectangle at `offset` in a layout: `(x, y, height)` from the top-left of the text box.
fn caret_of(layout: &Lay, offset: usize) -> (f32, f32, f32) {
    let cursor = Cursor::from_byte_index(layout, offset, Affinity::Downstream);
    let rect = cursor.geometry(layout, CARET_WIDTH);
    (rect.x0 as f32, rect.y0 as f32, (rect.y1 - rect.y0) as f32)
}

/// The caret rectangle at `byte_offset` in `text` laid out at `font_size` and wrapped to `width`: returns
/// `(x, y, height)` from the top-left of the text box. An offset past the end, or inside a character, is clamped to the
/// character before it.
pub fn caret_rect(text: &str, font_size: f32, width: f32, byte_offset: usize) -> (f32, f32, f32) {
    let layout = laid_out(text, font_size, width);
    caret_of(&layout, boundary(text, byte_offset))
}

/// Whether the caret at `byte_offset` is on the first visual line and on the last one: `(first, last)`. Wrapped lines
/// count, so a caret on the second line of a paragraph that wraps is not on the first.
pub fn line_edges(text: &str, font_size: f32, width: f32, byte_offset: usize) -> (bool, bool) {
    let layout = laid_out(text, font_size, width);
    let (_, y, height) = caret_of(&layout, boundary(text, byte_offset));
    let (_, end_y, _) = caret_of(&layout, text.len());
    let half = height * 0.5;
    (y < half, (y - end_y).abs() < half)
}

/// The byte offset nearest to horizontal position `x` on visual line `line` of `text` (counted from 0). A `line` past
/// the last one means the last line, so `usize::MAX` gives the end of the text's last line. Used by Up and Down to
/// keep the caret's column when it moves into a neighbouring block.
pub fn offset_at(text: &str, font_size: f32, width: f32, x: f32, line: usize) -> usize {
    if text.is_empty() {
        return 0;
    }
    let layout = laid_out(text, font_size, width);
    let last = layout.len().saturating_sub(1);
    let line = line.min(last);
    let y = layout.get(line).map_or(0.0, |l| {
        let metrics = l.metrics();
        (metrics.block_min_coord + metrics.block_max_coord) * 0.5
    });
    let cursor = Cursor::from_point(&layout, x, y);
    cursor.index().min(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_offset_inside_a_character_is_clamped() {
        // "é" is two bytes: offset 1 falls inside it and must not panic.
        let _ = caret_rect("é", 16.0, 200.0, 1);
    }

    #[test]
    fn an_empty_text_has_offset_zero() {
        assert_eq!(offset_at("", 16.0, 200.0, 50.0, 0), 0);
    }
}
