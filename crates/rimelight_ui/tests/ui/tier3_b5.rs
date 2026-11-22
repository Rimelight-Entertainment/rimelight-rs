//! P0.17 batch 5 tests: the virtual List and the Table with 10,000 rows. Everything runs in one test function, because the
//! testing backend can be installed once per process (as in `tier3_b4.rs`).
//!
//! Keys are real events (`Window::dispatch_event` with `KeyPressed` and `KeyReleased`). Clicks are the List's and the
//! Table's own press paths, exposed as test hooks. The header resize is a real drag: pointer down on the handle, a move,
//! and a release, at the handle's position computed from the header's own geometry.
//!
//! The caller's part (the wiring of callbacks to the Rust rules in `rimelight_ui_kit::table`) is written in this file. A window that shows static rows does not need it. The test checks that the virtual rows are laid out only
//! on screen (a row far from the view is not in the view), and it measures a frame time with 10,000 rows (see the end).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rimelight_ui_kit::table::{
    SAMPLE_COLUMNS, Selection, SelectionMode, SortState, clamp_width, sample_list_labels,
    sample_rows, sort_rows,
};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle as _, LogicalPosition, Model as _, ModelRc, SharedString, VecModel};

const ROWS: usize = 10_000;
const FRAMES: usize = 20;

pub use rimelight_ui_test_slint::tier3_b5::*;

fn list_model() -> Rc<VecModel<ListRow>> {
    Rc::new(VecModel::from(
        sample_list_labels(ROWS)
            .into_iter()
            .map(|label| ListRow {
                label: label.into(),
                selected: false,
            })
            .collect::<Vec<_>>(),
    ))
}

fn table_row(cells: [String; 4]) -> TableRow {
    TableRow {
        cells: ModelRc::new(VecModel::from(
            cells
                .into_iter()
                .map(SharedString::from)
                .collect::<Vec<_>>(),
        )),
        selected: false,
    }
}

fn table_model() -> Rc<VecModel<TableRow>> {
    Rc::new(VecModel::from(
        sample_rows(ROWS)
            .into_iter()
            .map(table_row)
            .collect::<Vec<_>>(),
    ))
}

fn column_model() -> Rc<VecModel<TableColumn>> {
    let widths = [200.0, 160.0, 140.0, 120.0];
    Rc::new(VecModel::from(
        SAMPLE_COLUMNS
            .iter()
            .zip(widths)
            .map(|(title, width)| TableColumn {
                title: (*title).into(),
                width,
                sortable: true,
            })
            .collect::<Vec<_>>(),
    ))
}

fn press(ui: &B5Form, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

/// The centre of the resize handle of column `column`: the 6 px handle at the right edge of its header cell.
fn handle_centre(ui: &B5Form, columns: &VecModel<TableColumn>, column: usize) -> LogicalPosition {
    let right: f32 = (0..=column)
        .map(|c| columns.row_data(c).expect("a column").width)
        .sum();
    let x = ui.get_test_table_x() + right - ui.get_test_handle_width() / 2.0;
    let y = ui.get_test_table_y() + ui.get_test_header_height() / 2.0;
    LogicalPosition::new(x, y)
}

/// A real drag on the resize handle of a column: press in the middle of the handle, move by `dx`, release.
fn drag_handle(ui: &B5Form, columns: &VecModel<TableColumn>, column: usize, dx: f32) {
    let start = handle_centre(ui, columns, column);
    let end = LogicalPosition::new(start.x + dx, start.y);
    let window = ui.window();
    window.dispatch_event(WindowEvent::PointerMoved { position: start });
    window.dispatch_event(WindowEvent::PointerPressed {
        position: start,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerMoved { position: end });
    window.dispatch_event(WindowEvent::PointerReleased {
        position: end,
        button: PointerEventButton::Left,
    });
}

/// Writes the flags the selection decided onto the rows that changed.
fn apply_list_flags(model: &VecModel<ListRow>, flags: &[bool]) {
    for (index, flag) in flags.iter().enumerate() {
        if let Some(mut row) = model.row_data(index)
            && row.selected != *flag
        {
            row.selected = *flag;
            model.set_row_data(index, row);
        }
    }
}

fn apply_table_flags(model: &VecModel<TableRow>, flags: &[bool]) {
    for (index, flag) in flags.iter().enumerate() {
        if let Some(mut row) = model.row_data(index)
            && row.selected != *flag
        {
            row.selected = *flag;
            model.set_row_data(index, row);
        }
    }
}

fn selected_indices(flags: impl Iterator<Item = bool>) -> Vec<usize> {
    flags
        .enumerate()
        .filter(|(_, selected)| *selected)
        .map(|(index, _)| index)
        .collect()
}

fn cell(model: &VecModel<TableRow>, row: usize, column: usize) -> String {
    model
        .row_data(row)
        .and_then(|r| r.cells.row_data(column))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

pub fn tier3_batch5_list_and_table() {
    // The software renderer, so the frame timing below can render a frame (the testing backend with the software renderer).
    crate::support::init();
    let ui = B5Form::new().expect("create the form");

    let list = list_model();
    let table = table_model();
    let columns = column_model();
    ui.set_list_rows(ModelRc::new(list.clone()));
    ui.set_table_rows(ModelRc::new(table.clone()));
    ui.set_columns(ModelRc::new(columns.clone()));
    ui.show().expect("show the form");

    // The caller's wiring. A List or Table pick goes through the selection rules, and the flags go back on the rows.
    let list_selection = Rc::new(RefCell::new(Selection::new(SelectionMode::Multi)));
    let list_picks = Rc::new(Cell::new(0usize));
    {
        let (model, selection, picks) = (list.clone(), list_selection.clone(), list_picks.clone());
        ui.on_list_picked(move |index, shift, ctrl| {
            picks.set(picks.get() + 1);
            let mut flags: Vec<bool> = model.iter().map(|r| r.selected).collect();
            selection
                .borrow_mut()
                .pick(&mut flags, index as usize, shift, ctrl);
            apply_list_flags(&model, &flags);
        });
    }
    let table_selection = Rc::new(RefCell::new(Selection::new(SelectionMode::Multi)));
    {
        let (model, selection) = (table.clone(), table_selection.clone());
        ui.on_table_picked(move |index, shift, ctrl| {
            let mut flags: Vec<bool> = model.iter().map(|r| r.selected).collect();
            selection
                .borrow_mut()
                .pick(&mut flags, index as usize, shift, ctrl);
            apply_table_flags(&model, &flags);
        });
    }
    let sort_state = Rc::new(Cell::new(SortState::default()));
    {
        let (model, state, weak) = (table.clone(), sort_state.clone(), ui.as_weak());
        ui.on_table_sort(move |column| {
            let column = column as usize;
            let next = state.get().request(column);
            state.set(next);
            let rows: Vec<TableRow> = model.iter().collect();
            model.set_vec(sort_rows(
                &rows,
                |row| row.cells.row_data(column).unwrap_or_default().to_string(),
                next.descending,
            ));
            if let Some(ui) = weak.upgrade() {
                ui.set_sort_column(column as i32);
                ui.set_sort_descending(next.descending);
            }
        });
    }
    {
        let cols = columns.clone();
        ui.on_table_resized(move |column, width| {
            let column = column as usize;
            if let Some(mut col) = cols.row_data(column) {
                col.width = clamp_width(width);
                cols.set_row_data(column, col);
            }
        });
    }

    // Virtualisation: at the top, the first rows are on screen and the 10,000th is not.
    assert!(ui.invoke_test_list_on_screen(0), "Entry 00001 is on screen");
    assert!(
        !ui.invoke_test_list_on_screen(ROWS as i32 - 1),
        "a far row is not on screen"
    );
    assert!(
        ui.invoke_test_table_row_on_screen(0),
        "Item 00001 is on screen"
    );
    assert!(!ui.invoke_test_table_row_on_screen(ROWS as i32 - 1));

    // The List keys. Tab gives the list the focus (it is the first Tab stop).
    press(&ui, Key::Tab);
    press(&ui, Key::DownArrow);
    assert_eq!(ui.get_list_focus(), 1, "Down moves one row");
    press(&ui, Key::End);
    assert_eq!(
        ui.get_list_focus(),
        ROWS as i32 - 1,
        "End reaches the last row"
    );
    assert!(
        ui.invoke_test_list_on_screen(ROWS as i32 - 1),
        "the last row is on screen"
    );
    assert!(
        !ui.invoke_test_list_on_screen(0),
        "the first row is off screen"
    );
    press(&ui, Key::Home);
    assert_eq!(ui.get_list_focus(), 0, "Home returns to the first row");
    press(&ui, Key::PageDown);
    assert!(
        ui.get_list_focus() > 1 && ui.get_list_focus() < ROWS as i32 - 1,
        "Page Down moves by a page, not to the end: {}",
        ui.get_list_focus()
    );
    press(&ui, Key::Home);
    press(&ui, Key::UpArrow);
    assert_eq!(ui.get_list_focus(), 0, "Up stops at the first row");

    // Space toggles the focus row; Return selects only it.
    press(&ui, Key::Space);
    assert_eq!(list_picks.get(), 1);
    assert!(list.row_data(0).unwrap().selected, "Space selects row 0");
    press(&ui, Key::Space);
    assert!(
        !list.row_data(0).unwrap().selected,
        "a second Space clears it"
    );
    press(&ui, Key::DownArrow);
    press(&ui, Key::DownArrow);
    press(&ui, Key::Return);
    assert_eq!(
        selected_indices(list.iter().map(|r| r.selected)),
        vec![2],
        "Return selects only the focus row"
    );

    // A click on a row selects only that row and calls the callback. Entry 00005 is row 4.
    ui.invoke_test_list_pick(4);
    assert_eq!(
        selected_indices(list.iter().map(|r| r.selected)),
        vec![4],
        "a click on Entry 00005 selects only it"
    );
    assert_eq!(ui.get_list_focus(), 4, "a click moves the focus row");

    // The Table: Tab moves from the list to the header. Down moves to the body.
    press(&ui, Key::Tab);
    press(&ui, Key::DownArrow);
    press(&ui, Key::End);
    assert_eq!(
        ui.get_table_focus(),
        ROWS as i32 - 1,
        "End reaches the last table row"
    );
    assert!(
        ui.invoke_test_table_row_on_screen(ROWS as i32 - 1),
        "the last table row is on screen"
    );
    assert!(
        !ui.invoke_test_table_row_on_screen(0),
        "the first table row is off screen"
    );
    press(&ui, Key::Home);
    assert_eq!(ui.get_table_focus(), 0);
    press(&ui, Key::Space);
    assert!(
        table.row_data(0).unwrap().selected,
        "Space selects the focus row"
    );
    press(&ui, Key::Space);
    assert!(!table.row_data(0).unwrap().selected);

    // Sort from the keyboard: the header, Right to Score (column 3), Return sorts ascending, Return again descending.
    ui.invoke_focus_table_header();
    press(&ui, Key::RightArrow);
    press(&ui, Key::RightArrow);
    press(&ui, Key::RightArrow);
    press(&ui, Key::Return);
    assert_eq!(ui.get_sort_column(), 3);
    assert!(!ui.get_sort_descending(), "the first sort is ascending");
    let scores: Vec<f64> = table
        .iter()
        .map(|r| r.cells.row_data(3).unwrap().parse::<f64>().unwrap())
        .collect();
    assert!(scores.windows(2).all(|w| w[0] <= w[1]), "ascending scores");
    assert_eq!(scores[0], 0.0);
    press(&ui, Key::Return);
    assert!(ui.get_sort_descending(), "the second sort is descending");
    assert_eq!(
        cell(&table, 0, 3),
        "999",
        "descending puts the largest score first"
    );
    let scores: Vec<f64> = table
        .iter()
        .map(|r| r.cells.row_data(3).unwrap().parse::<f64>().unwrap())
        .collect();
    assert!(scores.windows(2).all(|w| w[0] >= w[1]), "descending scores");

    // Sort from the mouse: a click on the Team header sorts ascending by team.
    ui.invoke_test_table_sort_click(1);
    assert_eq!(ui.get_sort_column(), 1);
    assert!(!ui.get_sort_descending());
    assert_eq!(
        cell(&table, 0, 1),
        "Design",
        "ascending by team starts with Design"
    );
    assert_eq!(table.row_count(), ROWS, "a sort keeps every row");

    // Column resize: a real drag on the Score handle widens the column by the drag distance, and a drag past the
    // minimum stops at the minimum (48 px).
    assert_eq!(columns.row_data(3).unwrap().width, 120.0);
    drag_handle(&ui, &columns, 3, 30.0);
    assert_eq!(
        columns.row_data(3).unwrap().width,
        150.0,
        "a 30 px drag widens by 30 px"
    );
    drag_handle(&ui, &columns, 3, -400.0);
    assert_eq!(
        columns.row_data(3).unwrap().width,
        rimelight_ui_kit::table::MIN_COLUMN_WIDTH,
        "a drag past the minimum stops at it"
    );

    // A click on the focus row of the table (the last one) selects only that row.
    press(&ui, Key::Tab);
    press(&ui, Key::End);
    ui.invoke_test_table_pick(ROWS as i32 - 1);
    let selected = selected_indices(table.iter().map(|r| r.selected));
    assert_eq!(
        selected,
        vec![ROWS - 1],
        "a click selects only the clicked row"
    );

    // Frame time with 10,000 rows. Each sample is one Page Down in the table and then one full software render of the
    // window (`Window::take_snapshot`). Only the render is timed.
    press(&ui, Key::Home);
    let mut frames: Vec<Duration> = Vec::with_capacity(FRAMES);
    for _ in 0..FRAMES {
        press(&ui, Key::PageDown);
        let started = Instant::now();
        ui.window().take_snapshot().expect("render a frame");
        frames.push(started.elapsed());
    }
    let millis: Vec<f64> = frames.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    let avg = millis.iter().sum::<f64>() / millis.len() as f64;
    let max = millis.iter().cloned().fold(f64::MIN, f64::max);
    let min = millis.iter().cloned().fold(f64::MAX, f64::min);
    println!("table 10k frame time, N={FRAMES}: avg {avg:.2} ms, max {max:.2} ms, min {min:.2} ms");
}
