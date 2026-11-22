//! S8 Dialog interaction tests, driven by real key and pointer events through
//! `Window::dispatch_event` and the forms' `click-*` hooks, which run the same callbacks a pointer click does. Nothing is
//! drawn; the testing backend holds the window. One test function, because the testing backend is installed once per
//! process.

use slint::ComponentHandle as _;
use slint::LogicalPosition;
use slint::SharedString;
use slint::platform::{Key, PointerEventButton, WindowEvent};

pub use rimelight_ui_test_slint::dialog::*;

// Slint runs `changed` handlers (here `opened` and `closed`, which count through `changed is-open`) on the next
// update, not inside the call that changes the property. Each step runs one before its counters are read.
fn settle() {
    slint::platform::update_timers_and_animations();
}

pub fn press_special<C: slint::ComponentHandle>(ui: &C, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
    settle();
}

pub fn click_at<C: slint::ComponentHandle>(ui: &C, at: LogicalPosition) {
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

pub fn dialog_focus_dismiss_and_confirm_with_real_events() {
    crate::support::init();

    // Open moves focus inside; Tab cycles through the stops and never reaches the page behind.
    let ui = DialogForm::new().expect("create the form");
    ui.show().expect("show the form");
    assert!(!ui.get_open(), "the dialog starts closed");

    ui.invoke_click_open();
    settle();
    assert!(ui.get_open(), "the trigger opens the dialog");
    assert_eq!(ui.get_opens(), 1, "opened fires once");
    assert!(!ui.get_trigger_focused(), "focus moved inside on open");
    assert!(
        ui.get_keys_focused(),
        "focus is on the first stop after open"
    );

    // Seven stops: the first stop, close, First, Second, Cancel, Save and the last stop. Two laps of Tab.
    let mut saw_end = false;
    for _ in 0..14 {
        press_special(&ui, Key::Tab);
        assert!(
            !ui.get_behind_focused(),
            "Tab never reaches the page behind"
        );
        saw_end |= ui.get_end_focused();
    }
    assert!(saw_end, "Tab reaches the last stop, then wraps");
    assert!(
        ui.get_keys_focused(),
        "after two laps focus is back on the first stop"
    );
    assert!(ui.get_open(), "Tab keeps the dialog open");

    // Shift+Tab on the first stop wraps to the last stop, and then walks backwards.
    press_special(&ui, Key::Backtab);
    assert!(
        ui.get_end_focused(),
        "Shift+Tab on the first stop wraps to the last"
    );
    assert!(
        !ui.get_behind_focused(),
        "Shift+Tab never reaches the page behind"
    );

    // Focus into First with a real Tab from the first stop, then Enter on it works.
    press_special(&ui, Key::Tab);
    press_special(&ui, Key::Tab);
    press_special(&ui, Key::Tab);
    press_special(&ui, Key::Return);
    assert_eq!(
        ui.get_first_clicks(),
        1,
        "Enter on the focused body control fires once"
    );

    // Escape closes, and focus returns to the opener.
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open(), "Escape closes a dismissible dialog");
    assert_eq!(ui.get_closes(), 1, "closed fires once");
    assert!(
        ui.get_trigger_focused(),
        "focus returns to the opener on close"
    );

    // A scrim click closes a dismissible dialog, and the page behind takes no click.
    ui.invoke_click_open();
    settle();
    assert!(ui.get_open());
    click_at(&ui, LogicalPosition::new(20.0, 560.0));
    assert!(!ui.get_open(), "a scrim click closes a dismissible dialog");
    assert_eq!(
        ui.get_behind_clicks(),
        0,
        "the press did not reach the page behind"
    );
    assert_eq!(ui.get_closes(), 2);

    // A press on the panel does not close it.
    ui.invoke_click_open();
    settle();
    assert!(ui.get_open());
    click_at(&ui, LogicalPosition::new(400.0, 300.0));
    assert!(ui.get_open(), "a press on the panel does not close it");

    // A footer action reports its index; the dialog stays open until the caller closes it.
    ui.invoke_click_save();
    settle();
    assert_eq!(ui.get_last_action(), 1, "Save reports index 1");
    assert!(
        ui.get_open(),
        "a footer action does not close the dialog by itself"
    );
    ui.invoke_close_dlg();
    settle();
    assert!(!ui.get_open(), "the caller closes the dialog from Rust");
    assert!(ui.get_trigger_focused(), "focus returns to the opener");

    // dismissible(false): Escape and a scrim click do nothing; the close control still closes.
    let locked = DialogForm::new().expect("create the locked form");
    locked.show().expect("show the locked form");
    locked.set_dismissible(false);
    locked.invoke_click_open();
    settle();
    assert!(locked.get_open());
    press_special(&locked, Key::Escape);
    assert!(
        locked.get_open(),
        "Escape does not close a non-dismissible dialog"
    );
    click_at(&locked, LogicalPosition::new(20.0, 560.0));
    assert!(
        locked.get_open(),
        "a scrim click does not close a non-dismissible dialog"
    );
    assert_eq!(locked.get_closes(), 0);
    locked.invoke_close_dlg();
    settle();
    assert!(
        !locked.get_open(),
        "the close control closes even when not dismissible"
    );
    assert_eq!(locked.get_closes(), 1);

    // ConfirmDialog runs in the same process: the testing backend is installed once per process.
    confirm_dialog_fires_once_and_cancel_does_not_confirm();
}

fn confirm_dialog_fires_once_and_cancel_does_not_confirm() {
    let ui = ConfirmForm::new().expect("create the confirm form");
    ui.show().expect("show the confirm form");

    // Confirm fires once and closes.
    ui.invoke_click_open();
    assert!(ui.get_open());
    ui.invoke_click_confirm();
    assert_eq!(ui.get_confirms(), 1, "confirm fires once");
    assert_eq!(ui.get_cancels(), 0, "confirm does not cancel");
    assert!(!ui.get_open(), "confirm closes the dialog");

    // Cancel fires cancelled once, never confirmed.
    ui.invoke_click_open();
    ui.invoke_click_cancel();
    assert_eq!(ui.get_cancels(), 1, "cancel fires once");
    assert_eq!(ui.get_confirms(), 1, "cancel does not confirm");
    assert!(!ui.get_open());

    // Escape is a cancel too, and fires it once (the close after a choice does not add one).
    ui.invoke_click_open();
    press_special(&ui, Key::Escape);
    assert_eq!(ui.get_cancels(), 2, "Escape cancels once");
    assert_eq!(ui.get_confirms(), 1);
}
