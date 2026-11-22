//! S5 NumberInput interaction tests: real key and pointer events, as in `text_fields.rs`.

use slint::ComponentHandle as _;
use slint::SharedString;
use slint::platform::{Key, WindowEvent};

pub use rimelight_ui_test_slint::number_input::*;

fn press_key(ui: &Form, text: char) {
    let text: SharedString = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn press_named(ui: &Form, key: Key) {
    let text: SharedString = key.into();
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

const ENTER: char = '\u{000a}';
const TAB: char = '\u{0009}';

pub fn number_input_behaviour_with_real_events() {
    crate::support::init();

    // Up and Down step the value from the field, after a click on it.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_quantity();
        press_named(&ui, Key::UpArrow);
        assert_eq!(ui.get_amount(), 6.0, "Up steps by one");
        press_named(&ui, Key::DownArrow);
        press_named(&ui, Key::DownArrow);
        assert_eq!(ui.get_amount(), 4.0, "Down steps by one");
    }

    // The labelled buttons step the value.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_increase();
        assert_eq!(ui.get_amount(), 6.0, "increase button steps up");
        ui.invoke_click_decrease();
        assert_eq!(ui.get_amount(), 5.0, "decrease button steps down");
    }

    // Clamp on Enter: typed 99 with a maximum of 10 becomes 10.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_quantity();
        for _ in 0..8 {
            press_key(&ui, '\u{0008}'); // backspace: clear "5"
        }
        type_text(&ui, "99");
        press_key(&ui, ENTER);
        assert_eq!(ui.get_amount(), 10.0, "Enter clamps to the maximum");
        assert_eq!(ui.get_amount_text().as_str(), "10");
    }

    // Clamp on focus loss: typed -4 with a minimum of 0 becomes 0 when focus moves away.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_quantity();
        for _ in 0..8 {
            press_key(&ui, '\u{0008}');
        }
        type_text(&ui, "-4");
        press_key(&ui, TAB);
        assert_eq!(
            ui.get_amount(),
            0.0,
            "leaving the field clamps to the minimum"
        );
    }

    // A click on the empty field focuses it, then typing reaches it.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        assert_eq!(
            ui.get_empty_text().as_str(),
            "0",
            "empty field shows its value"
        );
        ui.invoke_click_empty();
        for _ in 0..3 {
            press_key(&ui, '\u{0008}');
        }
        type_text(&ui, "7");
        press_key(&ui, ENTER);
        assert_eq!(
            ui.get_empty_value(),
            7.0,
            "a click on the empty field focuses it"
        );
    }

    // Letters are rejected; the field keeps its number.
    {
        let ui = Form::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_quantity();
        type_text(&ui, "ab");
        assert_eq!(
            ui.get_amount_text().as_str(),
            "5",
            "letters do not enter the field"
        );
    }
}
