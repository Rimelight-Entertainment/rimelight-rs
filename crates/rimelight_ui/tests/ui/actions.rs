//! S3 action components: Button, IconButton, ButtonGroup and Link, driven by real key events
//! (`WindowEvent::KeyPressed` and `KeyReleased` through `Window::dispatch_event`) and real pointer clicks
//! (the form's `click-*` hooks, which run the action a pointer click runs), on the testing backend. The same paths as
//! `tests/ui/keyboard.rs`.
//!
//! Checks the interaction rules of the Button, IconButton, ButtonGroup and Link:
//! click fires; disabled and loading do not; Tab reaches a button and Enter and Space activate it; a disabled button
//! is skipped by Tab; an icon button exposes its label; a button group segment click and Enter; a link click and Enter.
//!
//! One test function, because the testing backend can be installed once per process.

use slint::ComponentHandle as _;
use slint::SharedString;
use slint::platform::{Key, WindowEvent};

pub use rimelight_ui_test_slint::actions::*;

/// One key press and release, as an OS key event would deliver them.
fn press(ui: &ActionsForm, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn action_components_click_keys_and_labels() {
    crate::support::init();
    let ui = ActionsForm::new().expect("create the form");
    ui.show().expect("show the form");

    // Pointer: a click fires the enabled button. Disabled and loading do not fire.
    ui.invoke_click_save();
    assert_eq!(ui.get_save_clicks(), 1, "a click fires the button");
    ui.invoke_click_disabled();
    assert_eq!(
        ui.get_disabled_clicks(),
        0,
        "a disabled button ignores clicks"
    );
    ui.invoke_click_loading();
    assert_eq!(
        ui.get_loading_clicks(),
        0,
        "a loading button ignores clicks"
    );

    // A pointer click does not move keyboard focus (focus-on-click is false), so no focus ring is drawn for it.
    assert!(
        !ui.get_save_focused(),
        "a click must not give the focus ring"
    );

    // Icon button: exposes its label and its role to accessibility.
    // The label is read from the property (the accessibility role is not readable without the element lookup).
    assert_eq!(
        ui.get_close_label().as_str(),
        "Close",
        "the icon button exposes its label"
    );

    // Keyboard: the window has no focus until Tab. Tab reaches the first button, Enter activates it.
    press(&ui, Key::Tab);
    assert!(ui.get_save_focused(), "Tab reaches the first button");
    press(&ui, Key::Return);
    assert_eq!(
        ui.get_save_clicks(),
        2,
        "Enter activates the focused button"
    );
    press(&ui, Key::Space);
    assert_eq!(
        ui.get_save_clicks(),
        3,
        "Space activates the focused button"
    );

    // Tab skips the disabled button and lands on the loading button, which still takes focus but does not fire.
    press(&ui, Key::Tab);
    assert!(!ui.get_save_focused());
    press(&ui, Key::Return);
    assert_eq!(
        ui.get_loading_clicks(),
        0,
        "Enter does not fire a loading button"
    );
    assert_eq!(
        ui.get_disabled_clicks(),
        0,
        "Tab skipped the disabled button"
    );

    // The icon button takes Tab focus and Space activates it.
    press(&ui, Key::Tab);
    press(&ui, Key::Space);
    assert_eq!(ui.get_close_clicks(), 1, "Space activates the icon button");

    // Button group: one tab stop per segment. Enter chooses Day, Tab to Week, Space chooses Week.
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(ui.get_group_last(), 0, "Enter on the first segment");
    press(&ui, Key::Tab);
    press(&ui, Key::Space);
    assert_eq!(ui.get_group_last(), 1, "Space on the second segment");
    assert_eq!(ui.get_group_clicks(), 2);
    // A pointer click on the Month segment is a segment click with index 2.
    ui.invoke_click_month();
    assert_eq!(ui.get_group_last(), 2, "a click on the third segment");

    // Link: focus is still on Week (the pointer click on Month did not move it), so Tab goes to Month, then to the link.
    press(&ui, Key::Tab);
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(ui.get_link_clicks(), 1, "Enter activates the link");
    press(&ui, Key::Space);
    assert_eq!(ui.get_link_clicks(), 2, "Space activates the link");
    ui.invoke_click_link();
    assert_eq!(ui.get_link_clicks(), 3, "a click activates the link");
}
