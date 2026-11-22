//! S8 Banner interaction tests: the action and the dismiss fire once each, from a call and from the keyboard.
//! The test form exposes `click-action` and `click-dismiss`, which fire the banner's own callbacks. No element lookup
//! is used, so the test does not need the Slint debug info. Keys go through `Window::dispatch_event`.

use std::cell::Cell;
use std::rc::Rc;

use slint::ComponentHandle as _;
use slint::platform::{Key, WindowEvent};

pub use rimelight_ui_test_slint::banner::*;

fn press_special(ui: &BannerForm, key: Key) {
    let text: slint::SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn banner_action_and_dismiss_fire_once_each() {
    crate::support::init();
    let ui = BannerForm::new().expect("create the form");
    ui.show().expect("show the form");

    let actions = Rc::new(Cell::new(0));
    let dismisses = Rc::new(Cell::new(0));
    {
        let n = actions.clone();
        ui.on_action_clicked(move || n.set(n.get() + 1));
    }
    {
        let n = dismisses.clone();
        ui.on_dismiss_clicked(move || n.set(n.get() + 1));
    }

    ui.invoke_click_action();
    assert_eq!(actions.get(), 1, "the action button runs the action once");
    assert_eq!(dismisses.get(), 0, "the action does not dismiss");

    ui.invoke_click_dismiss();
    assert_eq!(
        dismisses.get(),
        1,
        "the dismiss button runs the dismiss once"
    );
    assert_eq!(actions.get(), 1, "the dismiss does not run the action");

    // Keyboard: Tab reaches the action first (the banner takes no focus of its own), then the dismiss. Enter on each runs
    // it once.
    press_special(&ui, Key::Tab);
    press_special(&ui, Key::Return);
    assert_eq!(actions.get(), 2, "Enter on the focused action runs it once");
    assert_eq!(dismisses.get(), 1);

    press_special(&ui, Key::Tab);
    press_special(&ui, Key::Return);
    assert_eq!(
        dismisses.get(),
        2,
        "Enter on the focused dismiss runs it once"
    );
    assert_eq!(actions.get(), 2);
}
