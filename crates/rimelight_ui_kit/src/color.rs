//! The Hex field of the ColorPicker (`ui/components/color-picker.slint`). Slint cannot read the characters of a string, so the
//! `ColorHex.parse-hex` callback is answered here. Every window that shows a ColorPicker or ColorField wires it once at startup.

use slint::Color;

/// The channels of a `#RGB` or `#RRGGBB` text. The leading `#` is optional, the digits may be either case, and spaces around
/// the text are ignored. Anything else (other lengths, non-hex characters, an empty text) is None.
pub fn parse_hex(text: &str) -> Option<(u8, u8, u8)> {
    let text = text.trim();
    let digits = text.strip_prefix('#').unwrap_or(text);
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = match digits.len() {
        // `#abc` is `#aabbcc`: each digit is doubled.
        3 => digits.chars().flat_map(|c| [c, c]).collect(),
        6 => digits.to_owned(),
        _ => return None,
    };
    let value = u32::from_str_radix(&full, 16).ok()?;
    Some(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

/// The colour of a hex text, opaque. A text that is not a hex colour is fully transparent, which the picker reads as refused.
pub fn parse_colour(text: &str) -> Color {
    match parse_hex(text) {
        Some((r, g, b)) => Color::from_rgb_u8(r, g, b),
        None => Color::from_argb_u8(0, 0, 0, 0),
    }
}

/// Emits `wire_hex(hex: &$hex)` in the calling module, where `$hex` is the app's generated `ColorHex` global. The emitted
/// function answers the global's `parse-hex` callback with [`parse_colour`]. Expand it in the app crate, where the
/// generated type is in scope, and call `wire_hex` once per window that shows a ColorPicker or ColorField.
#[macro_export]
macro_rules! wire_color_hex {
    ($hex:path $(,)?) => {
        /// Wires the `ColorHex` callback of one window to `parse_colour`.
        pub fn wire_hex(hex: &$hex) {
            hex.on_parse_hex(|text| $crate::color::parse_colour(text.as_str()));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{parse_colour, parse_hex};

    #[test]
    fn six_digits_read_red_green_blue_in_order() {
        assert_eq!(parse_hex("#1659C4"), Some((0x16, 0x59, 0xC4)));
        assert_eq!(parse_hex("#102030"), Some((0x10, 0x20, 0x30)));
    }

    #[test]
    fn three_digits_double_each_digit() {
        assert_eq!(parse_hex("#fa0"), Some((0xFF, 0xAA, 0x00)));
        assert_eq!(parse_hex("#abc"), Some((0xAA, 0xBB, 0xCC)));
        assert_eq!(parse_hex("#FFF"), Some((0xFF, 0xFF, 0xFF)));
    }

    #[test]
    fn the_hash_is_optional() {
        assert_eq!(parse_hex("1659c4"), Some((0x16, 0x59, 0xC4)));
        assert_eq!(parse_hex("abc"), Some((0xAA, 0xBB, 0xCC)));
    }

    #[test]
    fn case_does_not_matter() {
        assert_eq!(parse_hex("#aBcDeF"), parse_hex("#ABCDEF"));
        assert_eq!(parse_hex("#aBcDeF"), Some((0xAB, 0xCD, 0xEF)));
    }

    #[test]
    fn surrounding_spaces_are_ignored() {
        assert_eq!(parse_hex("  #00ff00 "), Some((0x00, 0xFF, 0x00)));
    }

    #[test]
    fn invalid_text_is_refused() {
        for text in [
            "", "#", "#12", "#1234", "#12345", "#1234567", "#GG0000", "#12 456", "#+12345", "red",
            "#-00000",
        ] {
            assert_eq!(parse_hex(text), None, "{text:?} should be refused");
        }
    }

    #[test]
    fn a_parsed_colour_is_opaque_and_a_refused_one_is_transparent() {
        let ok = parse_colour("#1659C4");
        assert_eq!(
            (ok.red(), ok.green(), ok.blue(), ok.alpha()),
            (0x16, 0x59, 0xC4, 0xFF)
        );
        assert_eq!(parse_colour("nope").alpha(), 0);
    }
}
