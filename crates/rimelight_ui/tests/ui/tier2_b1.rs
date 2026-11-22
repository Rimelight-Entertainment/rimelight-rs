//! P0.17 batch 1 tests: Card, Section, StatusDot, PresenceDot and Breadcrumbs. Everything runs in one test function,
//! because the testing backend can be installed once per process.
//!
//! Breadcrumbs is driven with test hooks that do what a segment's link does on a click, and with real key events
//! (`Window::dispatch_event` with `KeyPressed` and `KeyReleased`), the same path as `banner.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle as _, ModelRc, SharedString, VecModel};

pub use rimelight_ui_test_slint::tier2_b1::*;

fn press_special(ui: &B1Form, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn crumbs(items: &[&str]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .map(|s| SharedString::from(*s))
            .collect::<Vec<_>>(),
    ))
}

pub fn tier2_batch1_components_draw_and_breadcrumbs_navigate() {
    crate::support::init();
    let ui = B1Form::new().expect("create the form");
    ui.set_crumbs(crumbs(&["Projects", "Rimelight", "Launch plan"]));
    ui.show().expect("show the form");

    // Card, Section, StatusDot and PresenceDot hold their text and accessible names (read from their own properties).
    assert_eq!(ui.get_test_card_title(), "Card title", "card title is set");
    assert_eq!(
        ui.get_test_section_title(),
        "Section heading",
        "section heading is set"
    );
    assert_eq!(
        ui.get_test_sync_name(),
        "Sync state",
        "status dot has its name"
    );
    assert_eq!(
        ui.get_test_busy_name(),
        "Busy dot",
        "presence dot has its name"
    );

    // Breadcrumbs, uncollapsed: a click on each clickable segment reports its index once.
    let activations = Rc::new(RefCell::new(Vec::<i32>::new()));
    {
        let log = activations.clone();
        ui.on_activated(move |index: i32| log.borrow_mut().push(index));
    }

    ui.invoke_test_crumb_click(0);
    assert_eq!(
        *activations.borrow(),
        vec![0],
        "the first segment is index 0"
    );
    // Index 0 was clicked; "Projects" is index 0 in the list above.
    ui.invoke_test_crumb_click(1);
    assert_eq!(
        *activations.borrow(),
        vec![0, 1],
        "clicks report their index"
    );

    // The last segment is the current page: a click does nothing.
    ui.invoke_test_crumb_click(2);
    assert_eq!(
        *activations.borrow(),
        vec![0, 1],
        "the current segment does not activate"
    );

    // Keyboard: Tab reaches the first clickable segment, Enter activates it. The second Tab reaches the next one.
    press_special(&ui, Key::Tab);
    press_special(&ui, Key::Return);
    assert_eq!(
        *activations.borrow(),
        vec![0, 1, 0],
        "Tab then Enter on the first"
    );
    press_special(&ui, Key::Tab);
    press_special(&ui, Key::Return);
    assert_eq!(
        *activations.borrow(),
        vec![0, 1, 0, 1],
        "Tab then Enter on the second"
    );

    // Collapsed: five items with max-visible 3 keep the first, then the last two. The middle two are not drawn.
    ui.set_crumbs(crumbs(&[
        "Projects",
        "Rimelight",
        "Design",
        "Components",
        "Breadcrumbs",
    ]));
    ui.set_max_visible(3);
    activations.borrow_mut().clear();
    assert!(ui.invoke_test_ellipsis_drawn(), "the ellipsis is drawn");
    assert!(
        !ui.invoke_test_crumb_shown(1),
        "a collapsed segment is not drawn"
    );
    assert!(
        !ui.invoke_test_crumb_shown(2),
        "a collapsed segment is not drawn"
    );
    assert!(
        ui.invoke_test_crumb_shown(3),
        "the first visible tail item is drawn"
    );
    ui.invoke_test_crumb_click(3);
    assert_eq!(
        *activations.borrow(),
        vec![3],
        "the tail item keeps its index in the full list"
    );
    ui.invoke_test_crumb_click(0);
    assert_eq!(
        *activations.borrow(),
        vec![3, 0],
        "the first item stays clickable"
    );
}
