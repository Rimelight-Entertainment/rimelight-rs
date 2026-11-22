//! S2 task 1 (decision 0008): real key events into a Slint window, with no paid GUI Test Framework.
//!
//! What is real and what is simulated:
//! - Keys: every key is a `WindowEvent::KeyPressed` followed by `WindowEvent::KeyReleased`, dispatched with
//!   `slint::Window::dispatch_event`. That is the same entry point a windowing backend uses for an OS key event, and
//!   the events go through Slint's focus chain, the LineEdit and the FocusScope of the Button. The key text is the
//!   same character `i_slint_core::input::key_codes` uses for Tab, Return and Space.
//! - Focus of the LineEdit: the test form's `focus-edit` hook calls `LineEdit.focus()`, the same focus a pointer click
//!   on the field gives. No element lookup is used, so the test does not need the Slint debug info.
//! - Simulated: there is no OS window or OS input queue. The window is the testing backend's, so nothing is drawn
//!   and the clock is not the wall clock unless the test waits on it.

use slint::ComponentHandle as _;
use slint::SharedString;
use slint::platform::WindowEvent;

pub use rimelight_ui_test_slint::keyboard::*;

/// One key press and release, as an OS key event would deliver them. `text` is the Slint key text: a character,
/// or one of the `key_codes` constants such as Tab (U+0009), Return (U+000A) and Space (U+0020).
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

pub fn pointer_focus_typing_tab_and_activation() {
    crate::support::init();
    let ui = Form::new().expect("create the form");
    ui.show().expect("show the form");

    // Nothing is focused yet, so typing goes nowhere: a negative control for the assertions below.
    type_text(&ui, "x");
    assert_eq!(
        ui.get_typed().as_str(),
        "",
        "typing before any click must not reach the field"
    );

    // The LineEdit takes the focus (the hook stands in for the pointer click on it).
    ui.invoke_focus_edit();
    type_text(&ui, "hello");
    assert_eq!(
        ui.get_typed().as_str(),
        "hello",
        "the LineEdit received the typed keys"
    );
    assert!(!ui.get_button_focused());

    // Tab moves keyboard focus from the LineEdit to the Button.
    press_key(&ui, '\u{0009}');
    assert!(ui.get_button_focused(), "Tab must move focus to the Button");
    assert_eq!(
        ui.get_typed().as_str(),
        "hello",
        "Tab must not type into the field"
    );

    // Enter activates the focused Button.
    press_key(&ui, '\u{000a}');
    assert_eq!(
        ui.get_clicks(),
        1,
        "Enter must fire the Button's clicked callback"
    );

    // Space activates it too (the Button handles Space and Return in its FocusScope).
    press_key(&ui, ' ');
    assert_eq!(
        ui.get_clicks(),
        2,
        "Space must fire the Button's clicked callback"
    );
}
