//! S7 Tooltip interaction tests, driven by real key and pointer events through
//! `Window::dispatch_event` and `i_slint_backend_testing::mock_elapsed_time` for the hover delay. Nothing is drawn; the
//! testing backend holds the window. One test function, because the testing backend is installed once per process.

use i_slint_backend_testing::mock_elapsed_time;
use slint::ComponentHandle as _;
use slint::LogicalPosition;
use slint::SharedString;
use slint::platform::{Key, WindowEvent};

pub use rimelight_ui_test_slint::tooltip::*;

pub fn press_key<C: slint::ComponentHandle>(ui: &C, text: char) {
    let text: SharedString = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn press_special<C: slint::ComponentHandle>(ui: &C, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

/// A point on the first IconButton. The form lays its buttons out from the top-left corner with no padding, so this
/// point is inside the "Add" button without looking it up.
fn add_button_point() -> LogicalPosition {
    LogicalPosition::new(10.0, 10.0)
}

const TAB: char = '\u{0009}';

pub fn tooltip_shows_on_focus_and_hover_and_hides_on_escape_blur_and_leave() {
    crate::support::init();
    let ui = TipForm::new().expect("create the form");
    ui.show().expect("show the form");

    // Nothing shown before any focus or pointer.
    assert!(!ui.get_add_shown(), "no tooltip before focus or hover");

    // Keyboard focus shows the tooltip at once. Tab reaches the first IconButton.
    press_key(&ui, TAB);
    assert!(ui.get_add_shown(), "keyboard focus shows the tooltip");

    // Escape hides it, and the focus stays on the button.
    press_special(&ui, Key::Escape);
    assert!(!ui.get_add_shown(), "Escape hides the tooltip");

    // Moving focus away (Tab to the second button) hides it for good; the second has no text, so it never shows.
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(-5.0, -5.0),
    });
    press_key(&ui, TAB);
    assert!(!ui.get_add_shown(), "losing focus hides the tooltip");
    assert!(!ui.get_other_shown(), "an empty tooltip never shows");

    // Shift+Tab gives the first button focus again: the tooltip comes back, because Escape was cleared by the new focus.
    press_special(&ui, Key::Backtab);
    assert!(ui.get_add_shown(), "focus again shows the tooltip again");
    press_special(&ui, Key::Escape);
    assert!(!ui.get_add_shown(), "Escape hides it again");

    // Pointer hover: the tooltip waits for the hover delay (Tokens.motion-slow, 320 ms). Focus moves to the second button
    // first, so only the pointer can show the first tooltip.
    press_key(&ui, TAB);
    assert!(!ui.get_add_shown(), "the first button is no longer focused");
    let pointer_at = add_button_point();
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: pointer_at,
    });
    mock_elapsed_time(std::time::Duration::from_millis(100));
    assert!(
        !ui.get_add_shown(),
        "a passing pointer does not show the tooltip before the delay"
    );
    mock_elapsed_time(std::time::Duration::from_millis(400));
    assert!(
        ui.get_add_shown(),
        "a resting pointer shows the tooltip after the delay"
    );

    // Pointer leave hides it at once.
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(-5.0, -5.0),
    });
    assert!(!ui.get_add_shown(), "pointer leave hides the tooltip");
}
