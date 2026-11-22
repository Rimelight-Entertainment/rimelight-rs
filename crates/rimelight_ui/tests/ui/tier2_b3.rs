//! P0.17 batch 3 tests: Toolbar, FilterBar, Tree, ScrollArea and ResizablePanel. Everything runs in one test function,
//! because the testing backend can be installed once per process (as in `tier2_b2.rs`).
//!
//! Keys are real events (`Window::dispatch_event` with `KeyPressed` and `KeyReleased`). Clicks on the components' items
//! are the components' own click paths, exposed as test hooks. The panel handle is pressed with real pointer events at
//! its window position. Tab order follows the declaration order in the form below: Toolbar, FilterBar
//! (search, chip removes, Clear all), Tree, ScrollArea, ResizablePanel. The keyboard steps run first, then the pointer steps.
//! The Tree's rows are rebuilt from `rimelight_ui_kit::tree::visible_rows` whenever the test form reports a toggle.

use std::cell::RefCell;
use std::rc::Rc;

use rimelight_ui_kit::tree::{FlatNode, visible_rows};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle as _, LogicalPosition, ModelRc, SharedString, VecModel};

pub use rimelight_ui_test_slint::tier2_b3::*;

// The flat tree: Projects (open) has Design and Build; Archive (closed) has 2025.
const LABELS: [&str; 5] = ["Projects", "Design", "Build", "Archive", "2025"];

fn flat_tree() -> Vec<FlatNode> {
    vec![
        FlatNode {
            depth: 0,
            expanded: true,
        },
        FlatNode {
            depth: 1,
            expanded: false,
        },
        FlatNode {
            depth: 1,
            expanded: false,
        },
        FlatNode {
            depth: 0,
            expanded: false,
        },
        FlatNode {
            depth: 1,
            expanded: false,
        },
    ]
}

fn rows_model(nodes: &[FlatNode]) -> ModelRc<TreeRow> {
    let rows: Vec<TreeRow> = visible_rows(nodes)
        .into_iter()
        .map(|row| TreeRow {
            label: SharedString::from(LABELS[row.node]),
            depth: row.depth as i32,
            expanded: row.expanded,
            has_children: row.has_children,
            parent_row: row.parent_row.map_or(-1, |p| p as i32),
            node: row.node as i32,
        })
        .collect();
    ModelRc::new(VecModel::from(rows))
}

fn item(label: &str, enabled: bool, separator: bool) -> ToolbarItem {
    ToolbarItem {
        label: SharedString::from(label),
        icon: Default::default(),
        enabled,
        separator,
    }
}

fn chip(key: &str, label: &str) -> FilterChip {
    FilterChip {
        key: SharedString::from(key),
        label: SharedString::from(label),
    }
}

fn press(ui: &T3Form, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn type_text(ui: &T3Form, text: &str) {
    for c in text.chars() {
        let s = SharedString::from(c.to_string());
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: s.clone() });
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: s });
    }
}

/// Whether the tree draws a row with this label.
fn tree_shows(ui: &T3Form, label: &str) -> bool {
    (0..ui.invoke_test_row_count()).any(|i| ui.invoke_test_row_label(i) == label)
}

pub fn visible_rows_hide_children_of_collapsed_nodes() {
    let rows = visible_rows(&flat_tree());
    let nodes: Vec<usize> = rows.iter().map(|r| r.node).collect();
    assert_eq!(nodes, vec![0, 1, 2, 3], "2025 is under a closed branch");
    assert_eq!(rows[1].parent_row, Some(0));
    assert!(rows[0].has_children && !rows[1].has_children);
}

pub fn tier2_batch3_components_keys_and_callbacks() {
    crate::support::init();
    let ui = T3Form::new().expect("create the form");

    ui.set_tool_items(ModelRc::new(VecModel::from(vec![
        item("Add", true, false),
        item("Delete", true, false),
        item("", true, true),
        item("Find", true, false),
        item("Hide", false, false),
        item("Check", true, false),
    ])));
    ui.set_chip_list(ModelRc::new(VecModel::from(vec![
        chip("owner", "Owner: Ana"),
        chip("status", "Open"),
    ])));
    ui.set_query(SharedString::from(""));

    let nodes = Rc::new(RefCell::new(flat_tree()));
    ui.set_tree_rows(rows_model(&nodes.borrow()));

    let tool_log = Rc::new(RefCell::new(Vec::<i32>::new()));
    let query_log = Rc::new(RefCell::new(Vec::<String>::new()));
    let chip_log = Rc::new(RefCell::new(Vec::<String>::new()));
    let cleared_log = Rc::new(RefCell::new(0_usize));
    let select_log = Rc::new(RefCell::new(Vec::<i32>::new()));
    let toggle_log = Rc::new(RefCell::new(Vec::<(i32, bool)>::new()));
    let panel_log = Rc::new(RefCell::new(Vec::<f32>::new()));

    {
        let log = tool_log.clone();
        ui.on_tool_activated(move |i| log.borrow_mut().push(i));
    }
    {
        let log = query_log.clone();
        ui.on_query_changed(move |t| log.borrow_mut().push(t.to_string()));
    }
    {
        let log = chip_log.clone();
        ui.on_chip_removed(move |k| log.borrow_mut().push(k.to_string()));
    }
    {
        let log = cleared_log.clone();
        ui.on_cleared(move || *log.borrow_mut() += 1);
    }
    {
        let log = select_log.clone();
        ui.on_tree_selected_node(move |n| log.borrow_mut().push(n));
    }
    {
        // The caller owns the tree: a toggle changes the flat list and the rows are rebuilt from it.
        let log = toggle_log.clone();
        let state = nodes.clone();
        let weak = ui.as_weak();
        ui.on_tree_toggled(move |node, open| {
            log.borrow_mut().push((node, open));
            state.borrow_mut()[node as usize].expanded = open;
            if let Some(ui) = weak.upgrade() {
                ui.set_tree_rows(rows_model(&state.borrow()));
            }
        });
    }
    {
        let log = panel_log.clone();
        ui.on_panel_resized(move |w| log.borrow_mut().push(w));
    }

    ui.show().expect("show the form");

    // Every component draws its labels.
    assert_eq!(
        ui.invoke_test_tool_label(0),
        "Add",
        "toolbar button is drawn"
    );
    assert_eq!(
        ui.invoke_test_remove_label(0),
        "Remove Owner: Ana",
        "chip remove is drawn"
    );
    assert_eq!(
        ui.invoke_test_clear_all_label(),
        "Clear all",
        "Clear all is drawn"
    );
    assert!(tree_shows(&ui, "Projects"), "tree row is drawn");
    assert!(ui.get_test_handle_width() > 0.0, "panel handle is drawn");

    // Toolbar, keyboard. Tab reaches the toolbar; Right skips the separator and the disabled item and wraps; Enter and
    // Space activate; Home and End go to the ends.
    press(&ui, Key::Tab);
    assert_eq!(ui.get_tool_focus(), 0, "Tab lands on the first item");
    press(&ui, Key::RightArrow);
    assert_eq!(ui.get_tool_focus(), 1, "Right moves to the next item");
    press(&ui, Key::RightArrow);
    assert_eq!(ui.get_tool_focus(), 3, "Right skips the separator");
    press(&ui, Key::RightArrow);
    assert_eq!(ui.get_tool_focus(), 5, "Right skips the disabled item");
    press(&ui, Key::RightArrow);
    assert_eq!(ui.get_tool_focus(), 0, "Right wraps to the first item");
    press(&ui, Key::LeftArrow);
    assert_eq!(ui.get_tool_focus(), 5, "Left wraps to the last item");
    press(&ui, Key::LeftArrow);
    assert_eq!(ui.get_tool_focus(), 3, "Left skips the disabled item");
    assert!(tool_log.borrow().is_empty(), "arrows do not activate");
    press(&ui, Key::Return);
    assert_eq!(
        *tool_log.borrow(),
        vec![3],
        "Enter activates the focused item"
    );
    press(&ui, Key::Home);
    press(&ui, Key::Space);
    press(&ui, Key::End);
    press(&ui, Key::Return);
    assert_eq!(*tool_log.borrow(), vec![3, 0, 5], "Home, Space, End, Enter");

    // FilterBar, keyboard. Tab reaches the search field; typing reports each change.
    press(&ui, Key::Tab);
    type_text(&ui, "ad");
    assert_eq!(ui.get_query().as_str(), "ad");
    assert_eq!(*query_log.borrow(), vec!["a".to_string(), "ad".to_string()]);

    // With text in the field its clear button is a Tab stop, so two Tabs reach the first chip. Enter removes it; Tab then
    // Space removes the second; then Tab reaches Clear all.
    press(&ui, Key::Tab);
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(*chip_log.borrow(), vec!["owner".to_string()]);
    press(&ui, Key::Tab);
    press(&ui, Key::Space);
    assert_eq!(
        *chip_log.borrow(),
        vec!["owner".to_string(), "status".to_string()],
        "Space on the second remove"
    );
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(*cleared_log.borrow(), 1, "Enter on Clear all");

    // Tree, keyboard. Tab lands on row 0. Left on a leaf goes to the parent; Left on an open node asks to close it;
    // Right asks to open it again. Down, Enter selects. End then Right opens Archive; Home then Right enters Projects.
    press(&ui, Key::Tab);
    assert_eq!(ui.get_tree_focus(), 0);
    press(&ui, Key::DownArrow);
    assert_eq!(ui.get_tree_focus(), 1, "Down moves to Design");
    press(&ui, Key::LeftArrow);
    assert_eq!(ui.get_tree_focus(), 0, "Left on a leaf goes to the parent");
    press(&ui, Key::LeftArrow);
    assert_eq!(
        *toggle_log.borrow(),
        vec![(0, false)],
        "Left on an open node asks to close it"
    );
    assert!(
        !tree_shows(&ui, "Design"),
        "closing Projects hides its children"
    );
    press(&ui, Key::RightArrow);
    assert_eq!(*toggle_log.borrow(), vec![(0, false), (0, true)]);
    assert!(
        tree_shows(&ui, "Design"),
        "opening Projects shows its children"
    );
    press(&ui, Key::DownArrow);
    press(&ui, Key::Return);
    assert_eq!(*select_log.borrow(), vec![1], "Enter selects Design");
    press(&ui, Key::End);
    assert_eq!(ui.get_tree_focus(), 3, "End goes to the last visible row");
    press(&ui, Key::RightArrow);
    assert_eq!(
        *toggle_log.borrow().last().unwrap(),
        (3, true),
        "Right asks to open Archive"
    );
    assert_eq!(ui.get_tree_focus(), 3, "focus stays on Archive");
    press(&ui, Key::DownArrow);
    assert_eq!(ui.get_tree_focus(), 4, "Down reaches the new child");
    press(&ui, Key::Home);
    press(&ui, Key::RightArrow);
    assert_eq!(
        ui.get_tree_focus(),
        1,
        "Right on an open node enters its first child"
    );

    // ScrollArea, keyboard. Tab lands on the area; Down scrolls, End goes to the bottom, Home to the top.
    press(&ui, Key::Tab);
    press(&ui, Key::DownArrow);
    let partway = ui.get_scroll_fraction();
    assert!(
        partway > 0.0 && partway < 1.0,
        "Down scrolls partway: {partway}"
    );
    press(&ui, Key::End);
    assert!(
        (ui.get_scroll_fraction() - 1.0).abs() < 1e-6,
        "End is the bottom"
    );
    press(&ui, Key::Home);
    assert!(ui.get_scroll_fraction().abs() < 1e-6, "Home is the top");

    // ResizablePanel, keyboard. Tab lands on the handle; Right widens by one step, Left narrows, Home is the minimum,
    // End the maximum. A key that cannot move the size reports nothing.
    press(&ui, Key::Tab);
    press(&ui, Key::RightArrow);
    assert_eq!(*panel_log.borrow(), vec![256.0], "Right widens by space-4");
    press(&ui, Key::LeftArrow);
    assert_eq!(*panel_log.borrow(), vec![256.0, 240.0]);
    press(&ui, Key::Home);
    assert_eq!(ui.get_panel_size(), 120.0, "Home is the minimum");
    press(&ui, Key::LeftArrow);
    assert_eq!(
        panel_log.borrow().len(),
        3,
        "Left at the minimum reports nothing"
    );
    press(&ui, Key::End);
    assert_eq!(ui.get_panel_size(), 480.0, "End is the maximum");

    // Pointer, toolbar: a click activates an enabled item; a disabled one does nothing.
    ui.invoke_test_tool_click(3);
    ui.invoke_test_tool_click(4);
    assert_eq!(
        *tool_log.borrow(),
        vec![3, 0, 5, 3],
        "only the enabled click"
    );

    // Pointer, FilterBar: a remove button reports its key, and Clear all reports once more.
    ui.invoke_test_chip_remove(1);
    assert_eq!(chip_log.borrow().last().map(String::as_str), Some("status"));
    ui.invoke_test_clear_all();
    assert_eq!(*cleared_log.borrow(), 2);

    // Pointer, tree: a row click selects it; the chevron toggles without selecting.
    ui.invoke_test_row_click(2);
    assert_eq!(select_log.borrow().last().copied(), Some(2));
    ui.invoke_test_toggle_click(3);
    assert_eq!(
        toggle_log.borrow().last().copied(),
        Some((3, false)),
        "the chevron closes Archive"
    );
    assert!(!tree_shows(&ui, "2025"), "closing Archive hides 2025");

    // Pointer, panel: a press and release on the handle without a move changes nothing. The handle's window position
    // comes from the form, and the events are the ones a mouse sends.
    let centre = LogicalPosition::new(
        ui.get_test_handle_x() + ui.get_test_handle_width() / 2.0,
        ui.get_test_handle_y() + ui.get_test_handle_height() / 2.0,
    );
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position: centre });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: centre,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: centre,
        button: PointerEventButton::Left,
    });
    assert_eq!(ui.get_panel_size(), 480.0, "a click does not resize");
    assert_eq!(
        panel_log.borrow().len(),
        4,
        "only the End key changed the size after the first steps"
    );
}
