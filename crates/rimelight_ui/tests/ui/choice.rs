//! S5 choice control interaction tests: Checkbox, RadioGroup, Switch and Slider, driven by real key
//! and pointer events through `Window::dispatch_event`, and clicks through the forms' `click-*` hooks, as in
//! `text_fields.rs`. The slider drag and the locked-slider press use the slider's on-screen geometry (its absolute position
//! and size, read from the form's properties). Nothing is drawn; the testing backend holds the window.
//!
//! One test function, because the testing backend is installed once per process. Each block below uses a fresh window.

use slint::ComponentHandle as _;
use slint::LogicalPosition;
use slint::SharedString;
use slint::platform::{Key, PointerEventButton, WindowEvent};

pub use rimelight_ui_test_slint::choice::*;

pub fn press_key<C: slint::ComponentHandle>(ui: &C, text: char) {
    let text: SharedString = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

/// A real pointer press and release at a point in window coordinates.
fn click_at<C: slint::ComponentHandle>(ui: &C, at: LogicalPosition) {
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: PointerEventButton::Left,
    });
}

const TAB: char = '\u{0009}';
const ENTER: char = '\u{000a}';

pub fn choice_controls_with_real_key_and_pointer_events() {
    crate::support::init();

    // Checkbox: a click toggles it and fires changed with the new value.
    {
        let ui = CheckForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_newsletter();
        assert_eq!(ui.get_changes(), 1, "a click fires changed once");
        assert!(ui.get_last_checked(), "changed reports checked");
        assert_eq!(ui.get_newsletter_state(), CheckState::Checked);
        ui.invoke_click_newsletter();
        assert!(!ui.get_last_checked(), "a second click unchecks");
        assert_eq!(ui.get_newsletter_state(), CheckState::Unchecked);
        assert_eq!(ui.get_changes(), 2);
    }

    // Checkbox: Space toggles, Enter does not. Tab from the field before the box reaches it.
    {
        let ui = CheckForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_before();
        press_key(&ui, TAB);
        assert!(ui.get_newsletter_focused(), "Tab reaches the checkbox");
        press_key(&ui, ENTER);
        assert_eq!(ui.get_changes(), 0, "Enter does not toggle a checkbox");
        press_key(&ui, ' ');
        assert_eq!(ui.get_changes(), 1, "Space toggles");
        assert_eq!(ui.get_newsletter_state(), CheckState::Checked);
        press_key(&ui, TAB);
        assert!(
            !ui.get_newsletter_focused(),
            "the next Tab leaves the checkbox"
        );
    }

    // Checkbox: a click on a labelled box does not move keyboard focus onto it.
    {
        let ui = CheckForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_newsletter();
        assert!(
            !ui.get_newsletter_focused(),
            "a pointer click does not take keyboard focus"
        );
    }

    // Disabled: a click and Space are both ignored, and the box is not a Tab stop.
    {
        let ui = CheckForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_disabled();
        assert_eq!(
            ui.get_disabled_changes(),
            0,
            "a disabled checkbox ignores clicks"
        );
        ui.invoke_click_before();
        press_key(&ui, TAB);
        press_key(&ui, TAB);
        press_key(&ui, ' ');
        assert_eq!(
            ui.get_disabled_changes(),
            0,
            "keys never reach a disabled checkbox"
        );
    }

    // Indeterminate becomes checked on the first toggle, then unchecked on the next.
    {
        let ui = CheckForm::new().expect("create the form");
        ui.show().expect("show the form");
        assert_eq!(ui.get_mixed_state(), CheckState::Indeterminate);
        ui.invoke_click_mixed();
        assert_eq!(
            ui.get_mixed_state(),
            CheckState::Checked,
            "indeterminate becomes checked"
        );
        assert_eq!(ui.get_mixed_changes(), 1);
        ui.invoke_click_mixed();
        assert_eq!(
            ui.get_mixed_state(),
            CheckState::Unchecked,
            "checked becomes unchecked"
        );
    }

    // RadioGroup: one Tab stop. Tab from before the group lands on the selected option; the next Tab leaves the group.
    {
        let ui = RadioForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_before();
        press_key(&ui, TAB);
        assert!(ui.get_group_focused(), "Tab lands on the group");
        press_key(&ui, TAB);
        assert!(!ui.get_group_focused(), "the next Tab leaves the group");
    }

    // RadioGroup: Down and Right move to the next enabled option, skipping the disabled one, and wrap at the end. Up and
    // Left move back, wrapping at the start.
    {
        let ui = RadioForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_before();
        press_key(&ui, TAB);
        press_key(&ui, char::from(Key::DownArrow));
        assert_eq!(
            ui.get_value().as_str(),
            "gamma",
            "Down skips the disabled Beta"
        );
        assert_eq!(ui.get_changes(), 1);
        press_key(&ui, char::from(Key::DownArrow));
        assert_eq!(
            ui.get_value().as_str(),
            "alpha",
            "Down wraps to the first option"
        );
        press_key(&ui, char::from(Key::UpArrow));
        assert_eq!(
            ui.get_value().as_str(),
            "gamma",
            "Up wraps to the last option"
        );
        press_key(&ui, char::from(Key::LeftArrow));
        assert_eq!(
            ui.get_value().as_str(),
            "alpha",
            "Left moves back, skipping Beta"
        );
        press_key(&ui, char::from(Key::RightArrow));
        assert_eq!(ui.get_value().as_str(), "gamma", "Right moves forward");
        assert_eq!(
            ui.get_last_value().as_str(),
            "gamma",
            "changed reports the value"
        );
    }

    // RadioGroup: a click selects an enabled option; a click on a disabled option is ignored.
    {
        let ui = RadioForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_option(1);
        assert_eq!(
            ui.get_value().as_str(),
            "alpha",
            "a disabled option cannot be picked"
        );
        ui.invoke_click_option(2);
        assert_eq!(
            ui.get_value().as_str(),
            "gamma",
            "a click selects the option"
        );
        assert_eq!(ui.get_changes(), 1);
    }

    // Switch: a click, a click on the label, Space and Enter each toggle and fire changed.
    {
        let ui = SwitchForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_email();
        assert!(ui.get_on_state(), "a click turns the switch on");
        assert!(ui.get_last_on(), "changed reports on");
        assert_eq!(ui.get_changes(), 1);
        ui.invoke_click_before();
        press_key(&ui, TAB);
        assert!(ui.get_on_focused(), "Tab reaches the switch");
        press_key(&ui, ' ');
        assert!(!ui.get_on_state(), "Space toggles off");
        press_key(&ui, ENTER);
        assert!(ui.get_on_state(), "Enter toggles on");
        assert_eq!(ui.get_changes(), 3);
    }

    // Switch: disabled ignores a click and keys, and is not a Tab stop.
    {
        let ui = SwitchForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_locked();
        assert_eq!(
            ui.get_disabled_changes(),
            0,
            "a disabled switch ignores clicks"
        );
        assert!(!ui.get_disabled_state());
        ui.invoke_click_before();
        press_key(&ui, TAB);
        press_key(&ui, ' ');
        press_key(&ui, ENTER);
        assert_eq!(
            ui.get_disabled_changes(),
            0,
            "keys never reach a disabled switch"
        );
        assert!(!ui.get_disabled_state());
    }
    // Slider: Tab reaches it; arrows step by the step and clamp at the ends; Home and End go to the ends.
    {
        let ui = SliderForm::new().expect("create the form");
        ui.show().expect("show the form");
        ui.invoke_click_before();
        press_key(&ui, TAB);
        assert!(ui.get_vol_focused(), "Tab reaches the slider");
        press_key(&ui, char::from(Key::RightArrow));
        assert_eq!(ui.get_volume(), 50.0, "Right steps up by the step");
        press_key(&ui, char::from(Key::LeftArrow));
        assert_eq!(ui.get_volume(), 40.0, "Left steps down by the step");
        press_key(&ui, char::from(Key::End));
        assert_eq!(ui.get_volume(), 100.0, "End goes to the maximum");
        press_key(&ui, char::from(Key::RightArrow));
        assert_eq!(ui.get_volume(), 100.0, "Right clamps at the maximum");
        press_key(&ui, char::from(Key::Home));
        assert_eq!(ui.get_volume(), 0.0, "Home goes to the minimum");
        press_key(&ui, char::from(Key::LeftArrow));
        assert_eq!(ui.get_volume(), 0.0, "Left clamps at the minimum");
        assert_eq!(ui.get_changes(), 4, "only real changes fire changed");
    }

    // Slider: a pointer drag ends on a stepped value, and the thumb snaps during the drag (the value is stepped mid-drag).
    {
        let ui = SliderForm::new().expect("create the form");
        ui.show().expect("show the form");
        // The thumb is 16 px; its centre travels from 8 px to width - 8 px.
        let (x, y) = (ui.get_vol_x(), ui.get_vol_y());
        let (width, height) = (ui.get_vol_width(), ui.get_vol_height());
        let span = width - 16.0;
        let at = |fraction: f32| LogicalPosition::new(x + 8.0 + fraction * span, y + height / 2.0);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: at(0.37),
            button: PointerEventButton::Left,
        });
        assert_eq!(ui.get_volume(), 40.0, "a press at 37 per cent snaps to 40");
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: at(0.66) });
        assert_eq!(ui.get_volume(), 70.0, "the drag snaps to 70 while moving");
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: at(0.66),
            button: PointerEventButton::Left,
        });
        assert_eq!(ui.get_volume(), 70.0, "the drag ends on a stepped value");
    }

    // Slider: a disabled slider ignores the pointer and keys, and keeps its value.
    {
        let ui = SliderForm::new().expect("create the form");
        ui.show().expect("show the form");
        // A real press at the centre of the disabled slider.
        let (x, y) = (ui.get_locked_x(), ui.get_locked_y());
        let centre = LogicalPosition::new(
            x + ui.get_locked_width() / 2.0,
            y + ui.get_locked_height() / 2.0,
        );
        click_at(&ui, centre);
        assert_eq!(
            ui.get_locked_value(),
            30.0,
            "a disabled slider ignores clicks"
        );
    }
}
