//! S6 Combobox interaction tests, driven by real key and pointer events through
//! `Window::dispatch_event`, and clicks through the form's `click-fruit` hook, which runs the action a pointer click does.
//! Nothing is drawn; the testing backend holds the window. One test function, because the testing backend is installed
//! once per process.

use rimelight_ui_kit::select::{self, Entry};
use slint::ComponentHandle as _;
use slint::platform::{Key, WindowEvent};
use slint::{Model as _, ModelRc, SharedString, VecModel};

pub use rimelight_ui_test_slint::combobox::*;

fn entries(options: &ModelRc<SelectOption>) -> Vec<Entry> {
    options
        .iter()
        .map(|option| Entry {
            value: option.value.to_string(),
            label: option.label.to_string(),
            disabled: option.disabled,
            group: option.group.to_string(),
        })
        .collect()
}

/// Wires the `SelectNav` callbacks of this test window to the plain-Rust rules, the same wiring the app uses.
pub fn wire_nav<C: slint::ComponentHandle>(ui: &C, nav: &SelectNav) {
    let _ = ui;
    nav.on_next_enabled(|options, from, dir| select::next_enabled(&entries(&options), from, dir));
    nav.on_typeahead(|options, from, text| {
        select::typeahead(&entries(&options), from, text.as_str())
    });
    nav.on_heading_count(|options| select::heading_count(&entries(&options)));
    nav.on_index_of(|options, value| select::index_of(&entries(&options), value.as_str()));
    nav.on_filter_options(|options, query| {
        let kept: Vec<SelectOption> = options
            .iter()
            .filter(|option| select::matches_query(option.label.as_str(), query.as_str()))
            .collect();
        ModelRc::new(VecModel::from(kept))
    });
}

pub fn press_key<C: slint::ComponentHandle>(ui: &C, text: char) {
    let text: SharedString = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn press_special<C: slint::ComponentHandle>(ui: &C, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn type_text<C: slint::ComponentHandle>(ui: &C, text: &str) {
    for c in text.chars() {
        press_key(ui, c);
    }
}

const DOWN: Key = Key::DownArrow;
const ENTER: char = '\u{000a}';
const ESCAPE: char = '\u{001b}';

pub fn combobox_with_real_key_and_pointer_events() {
    crate::support::init();

    // A click on the trigger opens the menu and puts focus in the filter: typing narrows the list, the highlight goes
    // to the first result, and the menu stays open with no change.
    {
        let ui = ComboForm::new().expect("create the form");
        ui.invoke_focus_combo();
        wire_nav(&ui, &ui.global::<SelectNav>());
        ui.invoke_click_fruit();
        slint::platform::update_timers_and_animations();
        assert!(ui.get_menu_open(), "a click opens the menu");
        assert_eq!(ui.get_opens(), 1);
        assert_eq!(ui.get_results(), 5, "every option is listed before typing");
        type_text(&ui, "e");
        assert_eq!(
            ui.get_results(),
            3,
            "the typed e goes to the filter: Apple, Blueberry and Cherry match"
        );
        assert_eq!(
            ui.get_highlight(),
            0,
            "the highlight is on the first result"
        );
        assert!(ui.get_menu_open(), "typing does not close the menu");
        assert_eq!(ui.get_changes(), 0, "typing changes nothing");
    }

    // Down moves into the results and skips the disabled Apricot; Enter selects Banana once and closes.
    {
        let ui = ComboForm::new().expect("create the form");
        ui.invoke_focus_combo();
        wire_nav(&ui, &ui.global::<SelectNav>());
        ui.invoke_click_fruit();
        assert_eq!(
            ui.get_highlight(),
            0,
            "Apple is highlighted when the menu opens"
        );
        press_special(&ui, DOWN);
        assert_eq!(ui.get_highlight(), 2, "Down skips the disabled Apricot");
        press_key(&ui, ENTER);
        assert!(!ui.get_menu_open(), "Enter chooses and closes the menu");
        assert_eq!(ui.get_value().as_str(), "banana");
        assert_eq!(ui.get_last_value().as_str(), "banana");
        assert_eq!(ui.get_changes(), 1, "on-change fires once");
        assert_eq!(ui.get_closes(), 1);
    }

    // A filter that matches nothing shows the caller's no-results words; Enter then chooses nothing and keeps the menu.
    {
        let ui = ComboForm::new().expect("create the form");
        ui.invoke_focus_combo();
        wire_nav(&ui, &ui.global::<SelectNav>());
        ui.invoke_click_fruit();
        type_text(&ui, "kiwi");
        assert_eq!(
            ui.get_results(),
            0,
            "no result: the caller's no-results text is shown"
        );
        press_key(&ui, ENTER);
        assert!(
            ui.get_menu_open(),
            "Enter with no result keeps the menu open"
        );
        assert_eq!(ui.get_changes(), 0);
    }

    // Escape closes with no change and focus returns to the trigger, even after typing into the filter.
    {
        let ui = ComboForm::new().expect("create the form");
        ui.invoke_focus_combo();
        wire_nav(&ui, &ui.global::<SelectNav>());
        ui.invoke_click_fruit();
        type_text(&ui, "ban");
        assert_eq!(ui.get_results(), 1);
        press_key(&ui, ESCAPE);
        assert!(!ui.get_menu_open(), "Escape closes the menu");
        assert_eq!(ui.get_changes(), 0, "Escape changes nothing");
        assert_eq!(ui.get_value().as_str(), "", "the value is unchanged");
        assert_eq!(ui.get_closes(), 1);
        assert!(ui.get_focused(), "focus is back on the trigger");
    }
}
