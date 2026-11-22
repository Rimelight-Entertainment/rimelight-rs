//! S7 Menu interaction tests, driven by real key and pointer events through
//! `Window::dispatch_event`, and the trigger click and the row clicks through the form's `click-*` hooks, as in
//! `popover.rs`. Nothing is drawn; the testing backend holds the window. One test function, because the testing backend
//! is installed once per process.

use slint::ComponentHandle as _;
use slint::LogicalPosition;
use slint::SharedString;
use slint::platform::{Key, PointerEventButton, WindowEvent};

pub use rimelight_ui_test_slint::menu::*;

/// Wires the `MenuNav` callbacks of this test window to the plain-Rust rules in `rimelight_ui_kit::menu`. The test
/// window has its own generated types, so the conversion is here (an app expands the `wire_menu!` macro of `rimelight_ui_kit` in its own crate).
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

pub fn click_outside<C: slint::ComponentHandle>(ui: &C) {
    let at = LogicalPosition::new(400.0, 400.0);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: at,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: at,
        button: PointerEventButton::Left,
    });
}

pub fn menu_with_real_key_and_pointer_events() {
    crate::support::init();
    let ui = MenuForm::new().expect("create the form");
    // The Rust side wires the row callbacks of the window, as the gallery does.
    wire_nav(&ui.global::<MenuNav>());
    ui.show().expect("show the form");

    assert!(!ui.get_open(), "the menu starts closed");

    // A click on the trigger opens it, with the first selectable row highlighted: Undo (row 0) is disabled, so New (row 1) is.
    ui.invoke_click_trigger();
    assert!(ui.get_open(), "a click on the trigger opens the menu");
    assert_eq!(
        ui.get_highlight(),
        1,
        "the first press lands on New, not the disabled Undo"
    );
    assert!(
        !ui.get_trigger_focused(),
        "focus moved into the menu on open"
    );

    // Down skips the rule and the heading and lands on Word wrap; Up goes back.
    press_special(&ui, Key::DownArrow);
    assert_eq!(
        ui.get_highlight(),
        4,
        "Down skips the separator and the heading"
    );
    press_special(&ui, Key::UpArrow);
    assert_eq!(ui.get_highlight(), 1, "Up goes back to New");

    // Down and Enter choose Word wrap, once, and close the menu.
    press_special(&ui, Key::DownArrow);
    press_special(&ui, Key::Return);
    assert_eq!(ui.get_fired(), 1, "Enter fires the selection once");
    assert_eq!(
        ui.get_last_id().as_str(),
        "wrap",
        "the chosen id is the highlighted row's"
    );
    assert!(!ui.get_open(), "choosing an item closes the menu");
    assert!(
        ui.get_trigger_focused(),
        "focus returns to the trigger on close"
    );

    // Space on the trigger opens it again (the keyboard route); Down on the trigger also opens it.
    press_special(&ui, Key::Space);
    assert!(ui.get_open(), "Space on the focused trigger opens the menu");
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open(), "Escape closes the menu");
    assert!(
        ui.get_trigger_focused(),
        "focus returns to the trigger after Escape"
    );
    assert_eq!(ui.get_fired(), 1, "Escape fires nothing");
    press_special(&ui, Key::DownArrow);
    assert!(ui.get_open(), "Down on the focused trigger opens the menu");
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open());

    // A disabled row is never chosen by a click.
    ui.invoke_click_trigger();
    assert!(ui.get_open());
    // Item 0 is Undo, which is disabled.
    ui.invoke_click_item(0);
    assert_eq!(ui.get_fired(), 1, "a disabled row does not fire");
    assert!(ui.get_open(), "a disabled row does not close the menu");
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open());

    // Submenu: Down to More, Right opens it and highlights its first child (Duplicate). Down skips the disabled Print.
    ui.invoke_click_trigger();
    press_special(&ui, Key::DownArrow);
    press_special(&ui, Key::DownArrow);
    assert_eq!(ui.get_highlight(), 5, "More is highlighted");
    press_special(&ui, Key::RightArrow);
    assert_eq!(
        ui.get_expanded().as_str(),
        "more",
        "Right opens the submenu"
    );
    assert_eq!(
        ui.get_highlight(),
        6,
        "Right highlights the first child, Duplicate"
    );
    press_special(&ui, Key::DownArrow);
    assert_eq!(
        ui.get_highlight(),
        8,
        "Down skips the disabled child Print and lands on Delete"
    );
    press_special(&ui, Key::UpArrow);
    assert_eq!(ui.get_highlight(), 6, "Up goes back to Duplicate");

    // Left on a child closes the submenu and highlights More.
    press_special(&ui, Key::LeftArrow);
    assert_eq!(ui.get_expanded().as_str(), "", "Left closes the submenu");
    assert_eq!(ui.get_highlight(), 5, "and highlights More");
    assert!(ui.get_open(), "closing a submenu keeps the menu open");

    // Enter on More toggles it open; Escape closes the submenu first, and the menu stays open.
    press_special(&ui, Key::Return);
    assert_eq!(
        ui.get_expanded().as_str(),
        "more",
        "Enter on More expands it"
    );
    press_special(&ui, Key::Escape);
    assert_eq!(
        ui.get_expanded().as_str(),
        "",
        "Escape closes the open submenu"
    );
    assert!(ui.get_open(), "Escape on a submenu does not close the menu");
    assert_eq!(ui.get_fired(), 1, "submenu toggles fire nothing");

    // Right then Enter on the child Duplicate fires it once.
    press_special(&ui, Key::RightArrow);
    press_special(&ui, Key::Return);
    assert_eq!(ui.get_fired(), 2, "Enter on the child fires it once");
    assert_eq!(ui.get_last_id().as_str(), "dup");
    assert!(!ui.get_open(), "choosing a child closes the menu");

    // Escape with the menu closed leaves it closed. Outside click closes an open menu.
    ui.invoke_click_trigger();
    assert!(ui.get_open());
    click_outside(&ui);
    assert!(!ui.get_open(), "an outside click closes the menu");

    // Rust opens the same menu.
    ui.invoke_open_menu();
    assert!(ui.get_open(), "open from Rust opens the menu");
    assert_eq!(
        ui.get_highlight(),
        1,
        "and highlights the first selectable row"
    );
    // The keys reach the list without a click: Down moves the highlight.
    press_special(&ui, Key::DownArrow);
    assert_eq!(
        ui.get_highlight(),
        4,
        "the keyboard focus is in the list after a Rust open"
    );
    press_special(&ui, Key::Escape);
    assert!(!ui.get_open());
}
