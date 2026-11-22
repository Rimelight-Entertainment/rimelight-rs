//! S5 text field interaction tests: Input and its clear and password buttons, driven by real
//! key events through `Window::dispatch_event`, and clicks through the form's `click-*` hooks, which run the same actions
//! a pointer click does. Nothing is drawn; the testing backend holds the window.
//!
//! One test function per behaviour is avoided on purpose: the testing backend is installed once per process, so the
//! tests run in one function with a fresh window for each step.

use slint::ComponentHandle as _;
use slint::SharedString;
use slint::platform::WindowEvent;

pub use rimelight_ui_test_slint::text_fields::*;

fn press_key(ui: &Form, text: char) {
    let text: SharedString = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn type_text(ui: &Form, s: &str) {
    for ch in s.chars() {
        press_key(ui, ch);
    }
}

const TAB: char = '\u{0009}';
const ENTER: char = '\u{000a}';

pub fn input_behaviour_with_real_key_and_pointer_events() {
    crate::support::init();

    // Typing reaches the field after a click, and on-change reports each edit.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        type_text(&ui, "x");
        assert_eq!(
            ui.get_name_text().as_str(),
            "",
            "typing before any click must not reach the field"
        );

        ui.invoke_click_name();
        type_text(&ui, "hello");
        assert_eq!(ui.get_name_text().as_str(), "hello");
        assert_eq!(ui.get_changed_value().as_str(), "hello", "on-change");
        assert_eq!(ui.get_changes(), 5, "one change per typed character");

        // Enter submits the current text.
        press_key(&ui, ENTER);
        assert_eq!(ui.get_submits(), 1, "Enter fires submitted once");
        assert_eq!(ui.get_submitted_value().as_str(), "hello");
    }

    // Disabled and read-only fields reject typing.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_disabled();
        type_text(&ui, "zz");
        assert_eq!(
            ui.get_disabled_text().as_str(),
            "fixed",
            "disabled rejects typing"
        );

        ui.invoke_click_read_only();
        type_text(&ui, "zz");
        assert_eq!(
            ui.get_readonly_text().as_str(),
            "locked",
            "read-only rejects typing"
        );
    }

    // Clear by click: the labelled button empties the field.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_name();
        type_text(&ui, "abc");
        ui.invoke_click_clear_name();
        assert_eq!(
            ui.get_name_text().as_str(),
            "",
            "clear by click empties the field"
        );
        assert_eq!(
            ui.get_changed_value().as_str(),
            "",
            "clear reports the change"
        );
    }

    // Clear by keyboard: Tab from the field to the clear button, then Enter.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_name();
        type_text(&ui, "abc");
        press_key(&ui, TAB);
        press_key(&ui, ENTER);
        assert_eq!(
            ui.get_name_text().as_str(),
            "",
            "Tab then Enter clears the field"
        );
    }

    // Password: masked at first, toggled by click, and by Tab then Space.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        assert!(ui.get_secret_masked(), "a password starts masked");
        ui.invoke_click_reveal();
        assert!(
            ui.get_secret_revealed(),
            "the show button reveals the password"
        );
        ui.invoke_click_reveal();
        assert!(!ui.get_secret_revealed(), "the hide button masks it again");

        ui.invoke_click_secret();
        press_key(&ui, TAB);
        press_key(&ui, ' ');
        assert!(
            ui.get_secret_revealed(),
            "Tab to the toggle, then Space, reveals"
        );
        press_key(&ui, ' ');
        assert!(!ui.get_secret_revealed(), "Space again masks it");
    }
}
