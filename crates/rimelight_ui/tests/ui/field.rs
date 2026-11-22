//! S5 Field and Form tests: the error flag ORs with a child's own flag, and Tab moves through a
//! form of fields with real key events, as in `text_fields.rs`.

use slint::ComponentHandle as _;
use slint::SharedString;
use slint::platform::WindowEvent;

pub use rimelight_ui_test_slint::field::*;

fn press_key(ui: &Harness, text: char) {
    let text: SharedString = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

const TAB: char = '\u{0009}';

pub fn field_error_and_form_tab_order() {
    crate::support::init();

    // The error sets the child invalid. The child's own flag is kept: clearing the error does not clear it.
    {
        let ui = Harness::new().expect("create the harness");
        ui.show().expect("show the harness");
        assert!(!ui.get_name_invalid(), "no error, no own flag: valid");

        ui.set_error_text("Name is required".into());
        assert!(
            ui.get_name_invalid(),
            "the field error marks the child invalid"
        );

        ui.set_error_text("".into());
        assert!(!ui.get_name_invalid(), "clearing the error clears the mark");

        ui.set_own_invalid(true);
        assert!(ui.get_name_invalid(), "the child's own flag is honoured");
        ui.set_error_text("Too short".into());
        ui.set_error_text("".into());
        assert!(
            ui.get_name_invalid(),
            "an error that came and went does not clear the child's own flag"
        );
    }

    // Tab moves from field to field through the form, in order.
    {
        let ui = Harness::new().expect("create the harness");
        ui.show().expect("show the harness");
        ui.invoke_focus_name();
        press_key(&ui, TAB);
        assert!(ui.get_email_focused(), "Tab goes from Name to Email");
        press_key(&ui, TAB);
        assert!(ui.get_qty_focused(), "Tab goes from Email to Quantity");
    }
}
