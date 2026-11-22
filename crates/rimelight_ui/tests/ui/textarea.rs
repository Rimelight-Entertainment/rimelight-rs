//! S5 Textarea interaction tests: real key events through `Window::dispatch_event`, as in `text_fields.rs`. The focus
//! a pointer click gives a field is set by the form's `focus-*` hooks (`focus()` on the field), so no element lookup is
//! used and the test does not need the Slint debug info.

use slint::ComponentHandle as _;
use slint::SharedString;
use slint::platform::WindowEvent;

pub use rimelight_ui_test_slint::textarea::*;

fn press_key(ui: &Form, text: char) {
    let text: SharedString = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn type_text(ui: &Form, s: &str) {
    for ch in s.chars() {
        if ch == '\n' {
            press_key(ui, '\u{000a}');
        } else {
            press_key(ui, ch);
        }
    }
}

pub fn textarea_behaviour_with_real_key_and_pointer_events() {
    crate::support::init();

    // Typing after the field takes the focus; Enter inserts a line break instead of submitting.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_focus_notes();
        type_text(&ui, "a\nb");
        assert_eq!(
            ui.get_notes_text().as_str(),
            "a\nb",
            "Enter inserts a line break"
        );
    }

    // Read-only and disabled reject typing.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_focus_locked();
        type_text(&ui, "zz");
        assert_eq!(
            ui.get_locked_text().as_str(),
            "fixed",
            "read-only rejects typing"
        );
        ui.invoke_focus_disabled();
        type_text(&ui, "zz");
        assert_eq!(ui.get_off_text().as_str(), "off", "disabled rejects typing");
    }

    // Auto-grow: the reserved rows follow the line count, clamped to rows (min) and max-rows.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        assert_eq!(ui.get_notes_rows(), 2, "minimum rows");
        ui.set_notes_lines(3);
        assert_eq!(ui.get_notes_rows(), 3, "grows with the lines");
        ui.set_notes_lines(9);
        assert_eq!(ui.get_notes_rows(), 4, "stops at max-rows");
        ui.set_notes_lines(1);
        assert_eq!(ui.get_notes_rows(), 2, "shrinks back to rows");
    }
}
