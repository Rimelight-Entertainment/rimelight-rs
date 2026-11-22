//! S8 Sheet interaction tests, driven by real key and pointer events through
//! `Window::dispatch_event`; the trigger and close clicks call the test form's hooks, as in `banner.rs`. Nothing is drawn; the testing
//! backend holds the window. One test function, because the testing backend is installed once per process.

use slint::ComponentHandle as _;
use slint::LogicalPosition;
use slint::SharedString;
use slint::platform::{Key, PointerEventButton, WindowEvent};

pub use rimelight_ui_test_slint::sheet::*;

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

pub fn sheet_focus_dismiss_placement_and_motion() {
    crate::support::init();

    // Right sheet on a desktop window: open moves focus inside, Tab cycles and never reaches the page behind.
    let ui = SheetDesk::new().expect("create the desk");
    ui.show().expect("show the desk");
    ui.invoke_click_open();
    settle();
    assert!(ui.get_open(), "the trigger opens the sheet");
    assert_eq!(ui.get_opens(), 1);
    assert!(!ui.get_trigger_focused(), "focus moved inside on open");
    assert!(
        ui.get_keys_focused(),
        "focus is on the first stop after open"
    );
    assert_eq!(ui.get_panel_width(), 400.0, "md is 400 px wide");
    assert_eq!(ui.get_panel_height(), 600.0, "a side sheet is full height");
    assert!(
        ui.get_slide_duration() > 0,
        "the slide runs when motion is on"
    );

    // Stops: first, close, Only open, Apply and the last stop (five). Twelve Tabs go round more than twice.
    let mut saw_end = false;
    for _ in 0..12 {
        press_special(&ui, Key::Tab);
        assert!(
            !ui.get_behind_focused(),
            "Tab never reaches the page behind"
        );
        saw_end |= ui.get_end_focused();
    }
    assert!(saw_end, "Tab reaches the last stop, then wraps");

    // Five stops: 12 Tabs leave focus on "Only open"; three more reach the first stop.
    for _ in 0..3 {
        press_special(&ui, Key::Tab);
    }
    assert!(
        ui.get_keys_focused(),
        "three more Tabs reach the first stop"
    );
    // Shift+Tab on the first stop wraps to the last stop.
    press_special(&ui, Key::Backtab);
    assert!(
        ui.get_end_focused(),
        "Shift+Tab on the first stop wraps to the last"
    );
    assert!(
        !ui.get_behind_focused(),
        "Shift+Tab never reaches the page behind"
    );

    // Escape closes, and focus returns to the opener.
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open(), "Escape closes a dismissible sheet");
    assert_eq!(ui.get_closes(), 1);
    assert!(
        ui.get_trigger_focused(),
        "focus returns to the opener on close"
    );

    // A scrim click (left of the side sheet) closes it; the page behind takes no click.
    ui.invoke_click_open();
    settle();
    click_at(&ui, LogicalPosition::new(20.0, 500.0));
    assert!(!ui.get_open(), "a scrim click closes a dismissible sheet");
    assert_eq!(
        ui.get_behind_clicks(),
        0,
        "the press did not reach the page behind"
    );

    // dismissible(false): Escape and a scrim click do nothing.
    ui.set_dismissible(false);
    ui.invoke_click_open();
    press_special(&ui, Key::Escape);
    assert!(
        ui.get_open(),
        "Escape does not close a non-dismissible sheet"
    );
    click_at(&ui, LogicalPosition::new(20.0, 500.0));
    assert!(
        ui.get_open(),
        "a scrim click does not close a non-dismissible sheet"
    );
    ui.invoke_click_close();
    settle();
    assert!(
        !ui.get_open(),
        "the close control closes even when not dismissible"
    );
    ui.set_dismissible(true);

    // Left and bottom placements: the panel's size, and a scrim click closes.
    ui.set_placement(SheetPlacement::Left);
    ui.invoke_click_open();
    assert_eq!(ui.get_panel_width(), 400.0);
    assert_eq!(ui.get_panel_height(), 600.0);
    click_at(&ui, LogicalPosition::new(600.0, 500.0));
    assert!(!ui.get_open(), "a scrim click closes a left sheet");

    ui.set_placement(SheetPlacement::Bottom);
    ui.invoke_click_open();
    assert_eq!(ui.get_panel_width(), 800.0, "a bottom sheet is full width");
    assert_eq!(ui.get_panel_height(), 400.0, "a bottom sheet is md tall");
    click_at(&ui, LogicalPosition::new(20.0, 20.0));
    assert!(!ui.get_open(), "a scrim click closes a bottom sheet");

    // Reduced motion: the slide duration is zero.
    ui.global::<Tokens>().set_reduced_motion(true);
    assert_eq!(
        ui.get_slide_duration(),
        0,
        "reduced motion gives a zero slide"
    );
    ui.global::<Tokens>().set_reduced_motion(false);
    assert!(ui.get_slide_duration() > 0);

    // Phone width: a right sheet is capped to the 390 px window and runs full height.
    let phone = SheetPhone::new().expect("create the phone");
    phone.show().expect("show the phone");
    phone.invoke_click_open();
    assert!(phone.get_open());
    assert_eq!(
        phone.get_panel_width(),
        390.0,
        "a right sheet fits a 390 px phone"
    );
    assert_eq!(
        phone.get_panel_height(),
        700.0,
        "full height, no top offset"
    );
    phone.invoke_click_close();
    settle();
    assert!(
        !phone.get_open(),
        "the close control closes the phone sheet"
    );
}
