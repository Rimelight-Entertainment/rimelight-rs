//! S4 feedback layouts: EmptyState's action and ErrorState's retry. Each fires once per activation,
//! by a real pointer click (a test hook that fires the button's own callback) and by real key events (Tab, then Return or Space, through
//! `Window::dispatch_event`), on the testing backend. The same paths as `tests/ui/actions.rs`.
//!
//! One test function, because the testing backend can be installed once per process.

use slint::ComponentHandle as _;
use slint::SharedString;
use slint::platform::{Key, WindowEvent};

pub use rimelight_ui_test_slint::feedback::*;

/// One key press and release, as an OS key event would deliver them.
fn press(ui: &FeedbackForm, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn feedback_actions_fire_once_per_activation() {
    crate::support::init();
    let ui = FeedbackForm::new().expect("create the form");
    ui.show().expect("show the form");

    // Pointer: one click on each button fires its callback once.
    ui.invoke_click_compose();
    assert_eq!(ui.get_action_count(), 1, "a click fires the action once");
    assert_eq!(ui.get_retry_count(), 0);

    ui.invoke_click_retry();
    assert_eq!(ui.get_retry_count(), 1, "a click fires the retry once");
    assert_eq!(ui.get_action_count(), 1);

    // Keyboard: the first Tab reaches the action button; Return fires it once, not twice (press and release).
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(
        ui.get_action_count(),
        2,
        "Tab then Enter fires the action once"
    );
    assert_eq!(ui.get_retry_count(), 1);

    // Space on the same button fires it once more.
    press(&ui, Key::Space);
    assert_eq!(ui.get_action_count(), 3, "Space fires the action once");

    // The next Tab moves to the retry button; Return fires the retry once and leaves the action alone.
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(
        ui.get_retry_count(),
        2,
        "Tab then Enter fires the retry once"
    );
    assert_eq!(ui.get_action_count(), 3, "the action did not fire again");
}
