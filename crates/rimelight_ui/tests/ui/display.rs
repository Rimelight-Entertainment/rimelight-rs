//! S4 display tests: Badge capping, Tag remove by click, Enter and Space, and the Kbd key split.
//! Real key events (`WindowEvent::KeyPressed` and `KeyReleased` through `Window::dispatch_event`) and real pointer clicks
//! (the form's `click-remove-design` hook, which fires the tag's own `removed` callback), on the testing backend. The
//! logic rules are in `rimelight_ui_kit` (`display.rs`); the Slint components must agree with them, and this file checks that
//! for the values used. The plain tag's label is read from its property, not from the accessibility tree.
//!
//! One test function, because the testing backend can be installed once per process.

use rimelight_ui_kit::display;
use slint::ComponentHandle as _;
use slint::SharedString;
use slint::platform::{Key, WindowEvent};

pub use rimelight_ui_test_slint::display::*;

/// One key press and release, as an OS key event would deliver them.
fn press(ui: &DisplayForm, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn display_components_cap_remove_and_split() {
    crate::support::init();
    let ui = DisplayForm::new().expect("create the form");
    ui.show().expect("show the form");

    // Badge capping: the Slint label equals the Rust rule for every value that matters.
    let cases = [
        (0, 99),
        (7, 99),
        (99, 99),
        (100, 99),
        (1234, 999),
        (999, 999),
        (1000, 999),
    ];
    for (count, max) in cases {
        ui.set_probe_count(count);
        ui.set_probe_max(max);
        let expected = display::badge_label(count as u32, max as u32);
        assert_eq!(
            ui.get_probe_label().as_str(),
            expected,
            "badge {count} with max {max}"
        );
    }

    // Tag remove by pointer: a click on the Design remove button removes it, and only it.
    ui.invoke_click_remove_design();
    assert_eq!(ui.get_design_removed(), 1, "a click removes the tag");
    assert_eq!(ui.get_urgent_removed(), 0);

    // Keyboard: Tab reaches the first remove button. Enter removes, Space removes again.
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(ui.get_design_removed(), 2, "Enter on the remove button");
    press(&ui, Key::Space);
    assert_eq!(ui.get_design_removed(), 3, "Space on the remove button");

    // Tab moves to the next remove button; Enter removes the Urgent tag, not the Design one.
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(ui.get_urgent_removed(), 1, "Tab then Enter removes Urgent");
    assert_eq!(ui.get_design_removed(), 3);

    // The tag with no remove button shows its label (read from the component's property).
    assert_eq!(
        ui.get_plain_label().as_str(),
        "Plain",
        "the plain tag shows its label"
    );

    // Avatar stack overflow: the Slint arithmetic equals `avatar_overflow` for six people and each slot count.
    for max in [0, 1, 3, 4, 6, 8] {
        ui.set_stack_max(max);
        let (shown, overflow) = display::avatar_overflow(6, max as usize);
        assert_eq!(ui.get_stack_shown(), shown as i32, "shown with max {max}");
        assert_eq!(
            ui.get_stack_overflow(),
            overflow as i32,
            "overflow with max {max}"
        );
    }

    // Progress: the Slint fraction is clamped to 0 through 1, as `clamp_progress` says.
    for value in [-0.5_f32, 0.0, 0.25, 0.42, 1.0, 1.7] {
        ui.set_probe_value(value);
        assert!(
            (ui.get_probe_fraction() - display::clamp_progress(value)).abs() < 1e-6,
            "progress {value}"
        );
    }

    // Reduced motion: every motion token is zero, so the spinner, skeleton and progress sweep have no animation duration.
    let tokens = ui.global::<Tokens>();
    tokens.set_reduced_motion(true);
    // Slint maps a `duration` to whole milliseconds (i64) in Rust.
    assert_eq!(
        tokens.get_motion_loop(),
        0,
        "reduced motion makes the loop duration zero"
    );
    assert_eq!(display::motion_duration_ms(1200, true), 0);
    tokens.set_reduced_motion(false);
    assert_eq!(tokens.get_motion_loop(), 1200);
    assert_eq!(display::motion_duration_ms(1200, false), 1200);

    // Avatar initials: the story passes these, so they must equal the Rust rule.
    assert_eq!(display::initials("Ana Souza"), "AS");
    assert_eq!(display::initials("Eva"), "E");

    // Kbd: the key split of the stories (`parse_combo`), for the combos the stories show.
    assert_eq!(display::parse_combo("Ctrl+K"), vec!["Ctrl", "K"]);
    assert_eq!(
        display::parse_combo("Ctrl+Shift+P"),
        vec!["Ctrl", "Shift", "P"]
    );
    assert_eq!(display::parse_combo("Alt+Tab"), vec!["Alt", "Tab"]);
}
