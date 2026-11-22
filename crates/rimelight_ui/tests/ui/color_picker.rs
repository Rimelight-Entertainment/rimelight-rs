//! The ColorPicker's Hex field parses its text through the `ColorHex` global. The app wires that callback
//! with the `wire_color_hex!` macro of `rimelight_ui_kit`, expanded in the app crate; this checks the markup reaches the Rust parser.

use rimelight_ui_kit::color;
use slint::ComponentHandle as _;

pub use rimelight_ui_test_slint::color_picker::*;

pub fn color_picker_hex_callback_is_wired_to_the_parser() {
    crate::support::init();

    let ui = PickerWindow::new().expect("picker window");
    ui.global::<ColorHex>()
        .on_parse_hex(|text| color::parse_colour(text.as_str()));

    let ok = ui.global::<ColorHex>().invoke_parse_hex("#0f0".into());
    assert_eq!(
        (ok.red(), ok.green(), ok.blue(), ok.alpha()),
        (0x00, 0xFF, 0x00, 0xFF)
    );

    let refused = ui.global::<ColorHex>().invoke_parse_hex("#12345".into());
    assert_eq!(refused.alpha(), 0);
    assert_eq!(ui.get_changes(), 0);
}
