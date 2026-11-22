//! S7 ContextMenu interaction tests: a right press opens the menu at the pointer; the Menu key and
//! Shift+F10 open it at the focused target; the keyboard behaviour inside is the Menu's. Real key and pointer events
//! through `Window::dispatch_event`, and row clicks through the form's `click-item` hook. One test function, because the
//! testing backend is installed once per process.

use slint::ComponentHandle as _;
use slint::LogicalPosition;
use slint::SharedString;
use slint::platform::{Key, PointerEventButton, WindowEvent};

pub use rimelight_ui_test_slint::context_menu::*;

/// Wires the `MenuNav` callbacks of this test window to the plain-Rust rules in `rimelight_ui_kit::menu` (see
/// `menu.rs`, which has the same conversion for its own window types).
fn wire_nav(nav: &MenuNav) {
    use rimelight_ui_kit::menu::{self as rules, Entry, Kind, Row};
    use slint::Model as _;

    fn entries(items: &slint::ModelRc<MenuItem>) -> Vec<Entry> {
        items
            .iter()
            .map(|item| Entry {
                id: item.id.to_string(),
                kind: match item.kind {
                    MenuKind::Item => Kind::Item,
                    MenuKind::Separator => Kind::Separator,
                    MenuKind::Heading => Kind::Heading,
                    MenuKind::Submenu => Kind::Submenu,
                },
                disabled: item.disabled,
                parent: item.parent.to_string(),
            })
            .collect()
    }
    fn rows(rows: &slint::ModelRc<MenuRow>) -> Vec<Row> {
        rows.iter()
            .map(|row| Row {
                index: row.index,
                depth: row.depth,
                expanded: row.expanded,
            })
            .collect()
    }

    nav.on_flatten(|items, expanded| {
        let out: Vec<MenuRow> = rules::flatten(&entries(&items), expanded.as_str())
            .into_iter()
            .map(|row| MenuRow {
                index: row.index,
                depth: row.depth,
                expanded: row.expanded,
            })
            .collect();
        slint::ModelRc::new(slint::VecModel::from(out))
    });
    nav.on_next_row(|items, rs, from, dir| {
        rules::next_row(&entries(&items), &rows(&rs), from, dir)
    });
    nav.on_row_of_id(|items, rs, id| rules::row_of_id(&entries(&items), &rows(&rs), id.as_str()));
    nav.on_first_child_row(|items, rs, parent| {
        rules::first_child_row(&entries(&items), &rows(&rs), parent)
    });
    nav.on_list_height(|items, rs, item_height, rule_height| {
        rules::list_height(&entries(&items), &rows(&rs), item_height, rule_height)
    });
}

pub fn press_special<C: slint::ComponentHandle>(ui: &C, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

/// A right press and its release at a point inside the target (window coordinates).
pub fn right_press_at<C: slint::ComponentHandle>(ui: &C, x: f32, y: f32) {
    let at = LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: PointerEventButton::Right,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: PointerEventButton::Right,
    });
}

pub fn context_menu_with_real_key_and_pointer_events() {
    crate::support::init();
    let ui = ContextForm::new().expect("create the form");
    wire_nav(&ui.global::<MenuNav>());
    ui.show().expect("show the form");

    assert!(!ui.get_open(), "the context menu starts closed");

    // A right press on the target opens the menu at the pointer, with the first selectable row (Cut) highlighted.
    right_press_at(&ui, 120.0, 100.0);
    assert!(ui.get_open(), "a right press opens the menu");
    assert_eq!(ui.get_highlight(), 0, "Cut is highlighted");

    // The keyboard inside is the Menu's: Down skips the disabled Paste and the rule.
    press_special(&ui, Key::DownArrow);
    assert_eq!(ui.get_highlight(), 1, "Down goes to Copy");
    press_special(&ui, Key::DownArrow);
    assert_eq!(
        ui.get_highlight(),
        4,
        "Down skips Paste (disabled) and the rule"
    );

    // Enter on Pin to top fires once and closes the menu; focus returns to the target.
    press_special(&ui, Key::Return);
    assert_eq!(ui.get_fired(), 1, "the item fires once");
    assert_eq!(ui.get_last_id().as_str(), "pin");
    assert!(!ui.get_open(), "choosing closes the menu");
    assert!(
        ui.get_target_focused(),
        "focus returns to the target on close"
    );

    // Escape closes an open menu with no choice.
    right_press_at(&ui, 120.0, 100.0);
    assert!(ui.get_open(), "reopens on a right press");
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open(), "Escape closes the menu");
    assert_eq!(ui.get_fired(), 1, "Escape fires nothing");
    assert!(
        ui.get_target_focused(),
        "focus returns to the target after Escape"
    );

    // The Menu key opens it at the focused target (the target gets focus first, as a Tab would give it).
    ui.invoke_focus_target();
    press_special(&ui, Key::Menu);
    assert!(ui.get_open(), "the Menu key opens the context menu");
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open());

    // Shift+F10 opens it too: Shift goes down as its own key, then F10.
    ui.invoke_focus_target();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    press_special(&ui, Key::F10);
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
    assert!(ui.get_open(), "Shift+F10 opens the context menu");
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open());

    // F10 without Shift does not open it.
    ui.invoke_focus_target();
    press_special(&ui, Key::F10);
    assert!(!ui.get_open(), "F10 alone does not open the menu");

    // A click on the disabled Paste never fires.
    right_press_at(&ui, 120.0, 100.0);
    // Item 2 is Paste, which is disabled.
    ui.invoke_click_item(2);
    assert_eq!(ui.get_fired(), 1, "a disabled row does not fire");
    assert!(ui.get_open(), "and does not close the menu");

    // Delete, a danger item, fires once on a click.
    // Item 6 is Delete, a danger item.
    ui.invoke_click_item(6);
    assert_eq!(ui.get_fired(), 2, "Delete fires once");
    assert_eq!(ui.get_last_id().as_str(), "delete");
    assert!(!ui.get_open());
}
