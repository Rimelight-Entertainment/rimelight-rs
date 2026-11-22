//! S7 Popover interaction tests, driven by real key and pointer events through
//! `Window::dispatch_event`, and by test hooks that fire the trigger's and Save's own callbacks, as in `select.rs`. Nothing is drawn; the testing
//! backend holds the window. One test function, because the testing backend is installed once per process.

use slint::ComponentHandle as _;
use slint::LogicalPosition;
use slint::SharedString;
use slint::platform::{Key, PointerEventButton, WindowEvent};

pub use rimelight_ui_test_slint::popover::*;

// Slint runs `changed` handlers (here `opened` and `closed`, which count through `changed is-open`) on the next
// update, not inside the call that changes the property. Each step runs one before its counters are read.
fn settle() {
    slint::platform::update_timers_and_animations();
}

pub fn press_key<C: slint::ComponentHandle>(ui: &C, text: char) {
    let text: SharedString = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
    settle();
}

pub fn press_special<C: slint::ComponentHandle>(ui: &C, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
    settle();
}

pub fn click_outside<C: slint::ComponentHandle>(ui: &C) {
    // The empty corner of the window, far from the trigger and the panel.
    let at = LogicalPosition::new(390.0, 340.0);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: PointerEventButton::Left,
    });
    settle();
}

pub fn popover_with_real_key_and_pointer_events() {
    crate::support::init();
    let ui = PopoverForm::new().expect("create the form");
    ui.show().expect("show the form");

    // Closed at the start.
    assert!(!ui.get_open(), "the popover starts closed");

    // Click opens. Focus moves into the panel: the trigger no longer holds it.
    ui.invoke_click_trigger();
    settle();
    assert!(ui.get_open(), "a click on the trigger opens the panel");
    assert_eq!(ui.get_opens(), 1, "opened fires once");
    assert!(
        !ui.get_trigger_focused(),
        "focus moved into the panel on open"
    );

    // Focus is inside: typing reaches the Input in the panel, without a click on it.
    press_key(&ui, 'a');
    press_key(&ui, 'd');
    press_key(&ui, 'a');
    assert_eq!(
        ui.get_name().as_str(),
        "ada",
        "typing goes to the Input in the panel"
    );
    assert!(ui.get_open(), "typing keeps the panel open");

    // Escape closes, and focus returns to the trigger.
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open(), "Escape closes the panel");
    assert_eq!(ui.get_closes(), 1, "closed fires once");
    assert!(
        ui.get_trigger_focused(),
        "focus returns to the trigger on close"
    );

    // A click on the trigger while open closes the panel once, and does not reopen it.
    ui.invoke_click_trigger();
    settle();
    assert!(ui.get_open(), "reopens on click");
    assert_eq!(ui.get_opens(), 2);
    ui.invoke_click_trigger();
    settle();
    assert!(
        !ui.get_open(),
        "a click on the trigger while open closes it"
    );
    assert_eq!(ui.get_closes(), 2, "that close fires once");

    // Outside click closes.
    ui.invoke_click_trigger();
    settle();
    assert!(ui.get_open(), "the panel opens again");
    click_outside(&ui);
    assert!(!ui.get_open(), "an outside click closes the panel");
    assert_eq!(ui.get_closes(), 3);

    // A button inside fires once per click.
    ui.invoke_click_trigger();
    settle();
    assert!(ui.get_open());
    ui.invoke_click_save();
    assert_eq!(ui.get_saves(), 1, "a button inside fires once");

    // Rust controls the same state as markup.
    ui.invoke_close_pop();
    settle();
    assert!(!ui.get_open(), "close from Rust closes the panel");
    ui.invoke_open_pop();
    settle();
    assert!(ui.get_open(), "open from Rust opens the panel");
    ui.invoke_toggle_pop();
    settle();
    assert!(!ui.get_open(), "toggle from Rust closes an open panel");
    ui.invoke_toggle_pop();
    settle();
    assert!(ui.get_open(), "toggle from Rust opens a closed panel");
}
