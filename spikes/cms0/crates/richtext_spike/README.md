# richtext_spike

Question: can Slint 1.18.1 give an editable rich-text paragraph (bold, italic, underline,
strikethrough, inline code, mention chips) with working IME and phone keyboard?

Approach ("input sink overlay"): a `TextInput` sits exactly on top of a read-only `StyledText`
with the same font, size, x/y, width and wrap. The TextInput owns focus, keyboard, IME,
caret and selection; its text is transparent. Rust holds plain text plus byte-range marks and
regenerates the StyledText markdown on every edit.

Run from the `cms0` folder:

    cargo run --bin richtext_spike

Tick "Show input layer" to draw the TextInput text in red at 50% (misalignment becomes visible).

## What to check by hand

1. **Alignment with styled glyphs.** Select text, toggle B/Code, and type after bold or code.
   With "Show input layer" on, the red TextInput glyphs should sit on the black styled glyphs.
   Expected weak point: bold and code are wider than regular text, so the transparent
   TextInput's caret and selection will drift from the visible glyphs after styled runs.
2. **IME.** On Windows, switch to a Japanese or Pinyin IME and type. Check that the candidate
   window appears at the caret and that the preedit line in the debug text shows the
   composition.
3. **Dead keys (Portuguese).** Type `´` then `a` (and `~` then `a` for ã). Expect `á` and `ã`.
4. **Wrapping.** Type a long line so it wraps. The red layer and the caret must stay aligned
   across wrapped lines.
5. **Mention as a unit.** Insert a mention with "@ mention", then press Backspace inside and
   after it. The TextInput will almost certainly delete one character at a time. If so, note
   it: the mention is a styled range, not an atomic chip.

Also check Ctrl+B / Ctrl+I / Ctrl+U, and that toggling a mark with the toolbar keeps the
selection.

## Fallback to evaluate (not implemented)

If caret drift is unacceptable: hide the TextInput caret (`text-cursor-width: 0`) and draw
our own caret from a Rust-side text layout using the `parley` crate (the layout engine Slint
uses internally). That means laying out the same runs in Rust, mapping byte offsets to glyph
positions, and drawing the caret and selection rectangles ourselves. Keyboard, IME and
phone-keyboard input would still come from the TextInput.
