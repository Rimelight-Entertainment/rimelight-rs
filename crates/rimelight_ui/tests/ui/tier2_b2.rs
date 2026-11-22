//! P0.17 batch 2 tests: Tabs, Accordion, Stepper and Pagination. Everything runs in one test function, because the
//! testing backend can be installed once per process (as in `tier2_b1.rs`).
//!
//! Keys are real events (`Window::dispatch_event` with `KeyPressed` and `KeyReleased`). Clicks are the components' own
//! click paths, exposed as test hooks. Tab order follows the declaration order in the form below: Tabs, Stepper,
//! Pagination, Accordion. All keyboard steps run before the pointer steps, so a click cannot move the Tab sequence.

use std::cell::RefCell;
use std::rc::Rc;

use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle as _, ModelRc, SharedString, VecModel};

pub use rimelight_ui_test_slint::tier2_b2::*;

fn press(ui: &T2Form, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn press_n(ui: &T2Form, key: Key, n: usize) {
    for _ in 0..n {
        press(ui, key);
    }
}

fn tab_model(labels: &[&str]) -> ModelRc<ContentTab> {
    ModelRc::new(VecModel::from(
        labels
            .iter()
            .map(|s| ContentTab {
                label: SharedString::from(*s),
            })
            .collect::<Vec<_>>(),
    ))
}

fn step_model(labels: &[&str]) -> ModelRc<StepItem> {
    ModelRc::new(VecModel::from(
        labels
            .iter()
            .map(|s| StepItem {
                label: SharedString::from(*s),
            })
            .collect::<Vec<_>>(),
    ))
}

fn entry_model(items: &[(&str, &str)]) -> ModelRc<AccordionEntry> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .map(|(title, body)| AccordionEntry {
                title: SharedString::from(*title),
                body: SharedString::from(*body),
            })
            .collect::<Vec<_>>(),
    ))
}

pub fn tier2_batch2_components_keys_and_callbacks() {
    crate::support::init();
    let ui = T2Form::new().expect("create the form");
    ui.set_tab_list(tab_model(&["Overview", "Activity", "Settings"]));
    ui.set_step_list(step_model(&["Account", "Profile", "Team", "Review"]));
    ui.set_accordion_items(entry_model(&[
        ("Shipping", "Orders leave within two working days."),
        ("Returns", "Returns are accepted for thirty days."),
        ("Support", "The desk answers the same day."),
    ]));
    ui.show().expect("show the form");

    let tab_log = Rc::new(RefCell::new(Vec::<i32>::new()));
    let step_log = Rc::new(RefCell::new(Vec::<i32>::new()));
    let page_log = Rc::new(RefCell::new(Vec::<i32>::new()));
    let toggle_log = Rc::new(RefCell::new(Vec::<(i32, bool)>::new()));
    {
        let log = tab_log.clone();
        ui.on_tab_picked(move |i: i32| log.borrow_mut().push(i));
    }
    {
        let log = step_log.clone();
        ui.on_step_picked(move |i: i32| log.borrow_mut().push(i));
    }
    {
        let log = page_log.clone();
        ui.on_page_picked(move |i: i32| log.borrow_mut().push(i));
    }
    {
        let log = toggle_log.clone();
        ui.on_toggled(move |i: i32, open: bool| log.borrow_mut().push((i, open)));
    }

    // Every component draws its labels.
    assert_eq!(
        ui.invoke_test_tab_label(0),
        "Overview",
        "tab label is drawn"
    );
    assert_eq!(
        ui.invoke_test_step_label(0),
        "Account",
        "step label is drawn"
    );
    assert_eq!(
        ui.invoke_test_accordion_title(0),
        "Shipping",
        "accordion header is drawn"
    );
    assert!(ui.invoke_test_page_shown(7), "page number is drawn");

    // Tabs, keyboard. Tab reaches the list; arrows move the focus only; Enter and Space activate.
    press(&ui, Key::Tab);
    press(&ui, Key::RightArrow);
    assert_eq!(ui.get_tab_active(), 0, "an arrow does not activate");
    assert!(
        tab_log.borrow().is_empty(),
        "an arrow does not fire activated"
    );
    press(&ui, Key::Return);
    assert_eq!(ui.get_tab_active(), 1, "Enter activates the focused tab");
    assert_eq!(*tab_log.borrow(), vec![1]);
    press(&ui, Key::End);
    press(&ui, Key::Space);
    assert_eq!(
        ui.get_tab_active(),
        2,
        "End then Space activates the last tab"
    );
    press(&ui, Key::Home);
    press(&ui, Key::LeftArrow);
    press(&ui, Key::Return);
    assert_eq!(
        ui.get_tab_active(),
        2,
        "Left from the first tab wraps to the last"
    );
    assert_eq!(*tab_log.borrow(), vec![1, 2, 2]);
    press(&ui, Key::Home);
    press(&ui, Key::Return);
    assert_eq!(
        ui.get_tab_active(),
        0,
        "Home then Enter activates the first tab"
    );
    assert_eq!(*tab_log.borrow(), vec![1, 2, 2, 0]);

    // Stepper, keyboard: the done step is the first Tab stop after the list. Enter and Space activate it.
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    press(&ui, Key::Space);
    assert_eq!(
        *step_log.borrow(),
        vec![0, 0],
        "Enter and Space on a done step"
    );

    // Pagination, keyboard: from the done step, Previous, 1, 5, 6 and 7 are the Tab stops at page 6. Enter on 7.
    press_n(&ui, Key::Tab, 5);
    press(&ui, Key::Return);
    assert_eq!(ui.get_page_now(), 7, "Enter on the page 7 button");
    assert_eq!(*page_log.borrow(), vec![7]);

    // Accordion, single, keyboard. The focus is on the slot that now shows 8. Tab goes past the ellipsis to 12, then Next,
    // then the first header. Enter opens it; Tab then Space opens the second, which closes the first; Enter closes it.
    press_n(&ui, Key::Tab, 3);
    press(&ui, Key::Return);
    assert_eq!(ui.get_open_index(), 0, "Enter opens the first header");
    assert_eq!(*toggle_log.borrow(), vec![(0, true)]);
    press(&ui, Key::Tab);
    press(&ui, Key::Space);
    assert_eq!(
        ui.get_open_index(),
        1,
        "single mode: opening one closes the other"
    );
    assert_eq!(*toggle_log.borrow(), vec![(0, true), (1, true)]);
    press(&ui, Key::Return);
    assert_eq!(
        ui.get_open_index(),
        -1,
        "Enter on the open header closes it"
    );
    assert_eq!(*toggle_log.borrow(), vec![(0, true), (1, true), (1, false)]);

    // Pointer, tabs: a click activates the tab once.
    ui.invoke_test_tab_click(1);
    assert_eq!(ui.get_tab_active(), 1, "a click activates the tab");
    assert_eq!(tab_log.borrow().last().copied(), Some(1));

    // Pointer, stepper: a click on a done step activates it; a click on the current step does nothing.
    ui.invoke_test_step_click(0);
    ui.invoke_test_step_click(1);
    assert_eq!(
        *step_log.borrow(),
        vec![0, 0, 0],
        "only done steps activate"
    );

    // Pointer, pagination: at page 7 the slots are 1, …, 6, 7, 8, …, 12. Clicking 8, then 12, picks them.
    ui.invoke_test_page_click(8);
    ui.invoke_test_page_click(12);
    assert_eq!(ui.get_page_now(), 12);
    assert_eq!(*page_log.borrow(), vec![7, 8, 12]);
    ui.invoke_test_page_click(12);
    assert_eq!(
        *page_log.borrow(),
        vec![7, 8, 12],
        "the current page is not reported"
    );
    ui.invoke_test_next();
    assert_eq!(
        *page_log.borrow(),
        vec![7, 8, 12],
        "Next does nothing on the last page"
    );
    assert_eq!(ui.get_page_now(), 12);
    assert!(
        ui.invoke_test_ellipsis_drawn(),
        "an ellipsis is drawn between the pages"
    );

    // Pointer, accordion, single: a click opens, a second click closes. The header asks for the state it does not have:
    // a row is open exactly when `open-index` is its index in single mode.
    ui.invoke_test_accordion_request(0, ui.get_open_index() != 0);
    assert_eq!(ui.get_open_index(), 0, "a click opens the first item");
    ui.invoke_test_accordion_request(0, ui.get_open_index() != 0);
    assert_eq!(ui.get_open_index(), -1, "a second click closes it");

    // Pointer, accordion, multiple: two items open at once, and open-index is not used. Each item's open state is
    // its own, so the test asks for the state the item is in: Shipping is closed, Returns is closed.
    ui.set_multiple(true);
    ui.invoke_test_accordion_request(0, true);
    ui.invoke_test_accordion_request(1, true);
    assert_eq!(
        ui.get_open_index(),
        -1,
        "multiple mode leaves open-index alone"
    );
    let log = toggle_log.borrow();
    let tail = &log[log.len() - 2..];
    assert_eq!(tail, &[(0, true), (1, true)], "both items report open");
    drop(log);
    ui.invoke_test_accordion_request(0, false);
    assert_eq!(
        toggle_log.borrow().last().copied(),
        Some((0, false)),
        "Shipping closes on its own"
    );
}
