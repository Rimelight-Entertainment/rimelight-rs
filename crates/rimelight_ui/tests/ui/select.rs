//! S6 Select interaction tests, driven by real key and pointer events through
//! `Window::dispatch_event`, and clicks through the forms' `click-*` hooks, which run the same actions a pointer click
//! does. Nothing is drawn; the testing backend holds the window. One test function, because the testing backend is
//! installed once per process.

use rimelight_ui_kit::select::{self, Entry};
use slint::ComponentHandle as _;
use slint::platform::{Key, WindowEvent};
use slint::{Model as _, ModelRc, SharedString, VecModel};

pub use rimelight_ui_test_slint::select::*;

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

/// Wires the `SelectNav` search callbacks of one window to the plain-Rust rules in `select`.
pub fn wire_nav<C: slint::ComponentHandle>(ui: &C, nav: &SelectNav) {
    let _ = ui;
    nav.on_next_enabled(|options, from, dir| select::next_enabled(&entries(&options), from, dir));
    nav.on_typeahead(|options, from, text| {
        select::typeahead(&entries(&options), from, text.as_str())
    });
    nav.on_heading_count(|options| select::heading_count(&entries(&options)));
    nav.on_index_of(|options, value| select::index_of(&entries(&options), value.as_str()));
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

const DOWN: Key = Key::DownArrow;
const ENTER: char = '\u{000a}';
const SPACE: char = ' ';
const ESCAPE: char = '\u{001b}';
const TAB: char = '\u{0009}';

pub fn select_with_real_key_and_pointer_events() {
    crate::support::init();

    // Click on the trigger opens the menu; a second click closes it with no change.
    {
        let ui = FruitForm::new().expect("create the form");
        ui.invoke_focus_fruit();
        wire_nav(&ui, &ui.global::<SelectNav>());
        ui.invoke_click_fruit();
        // Slint runs the `opened` handler on the next update pass, not inside the click.
        slint::platform::update_timers_and_animations();
        assert!(ui.get_menu_open(), "a click opens the menu");
        assert_eq!(ui.get_opens(), 1);
        ui.invoke_click_fruit();
        assert!(!ui.get_menu_open(), "a second click closes the menu");
        assert_eq!(
            ui.get_changes(),
            0,
            "closing without a choice changes nothing"
        );
    }

    // Enter opens; Down skips the disabled Apricot; Enter chooses Banana, closes, and fires changed once.
    {
        let ui = FruitForm::new().expect("create the form");
        ui.invoke_focus_fruit();
        wire_nav(&ui, &ui.global::<SelectNav>());
        press_special(&ui, DOWN);
        assert!(!ui.get_menu_open(), "arrow keys do nothing while closed");
        press_key(&ui, ENTER);
        assert!(ui.get_menu_open(), "Enter opens the menu");
        assert_eq!(
            ui.get_highlight(),
            0,
            "nothing chosen: the highlight starts on the first enabled option"
        );
        press_special(&ui, DOWN);
        assert_eq!(ui.get_highlight(), 2, "Down skips the disabled Apricot");
        press_key(&ui, ENTER);
        assert!(!ui.get_menu_open(), "Enter chooses and closes");
        assert_eq!(ui.get_value().as_str(), "banana");
        assert_eq!(ui.get_changes(), 1, "changed fires once");
        assert_eq!(ui.get_selected(), 2);
    }

    // Escape closes with no change, and focus stays on the trigger.
    {
        let ui = FruitForm::new().expect("create the form");
        ui.invoke_focus_fruit();
        wire_nav(&ui, &ui.global::<SelectNav>());
        ui.invoke_click_fruit();
        assert!(ui.get_focused(), "the trigger has focus after a click");
        press_special(&ui, DOWN);
        press_key(&ui, ESCAPE);
        assert!(!ui.get_menu_open(), "Escape closes the menu");
        assert_eq!(ui.get_changes(), 0, "Escape changes nothing");
        assert!(ui.get_focused(), "focus is on the trigger after Escape");
    }

    // Tab closes the menu with no change.
    {
        let ui = FruitForm::new().expect("create the form");
        ui.invoke_focus_fruit();
        wire_nav(&ui, &ui.global::<SelectNav>());
        press_key(&ui, ENTER);
        press_key(&ui, TAB);
        assert!(!ui.get_menu_open());
        assert_eq!(ui.get_changes(), 0);
    }

    // Clicking an option selects it, skips a disabled option, and closes the menu.
    {
        let ui = FruitForm::new().expect("create the form");
        ui.invoke_focus_fruit();
        wire_nav(&ui, &ui.global::<SelectNav>());
        ui.invoke_click_fruit();
        ui.invoke_click_option(1);
        assert_eq!(
            ui.get_changes(),
            0,
            "a disabled option is not selectable by click"
        );
        assert!(
            ui.get_menu_open(),
            "the menu stays open after a click on a disabled option"
        );
        ui.invoke_click_option(3);
        assert_eq!(ui.get_value().as_str(), "blueberry");
        assert_eq!(ui.get_changes(), 1);
        assert!(!ui.get_menu_open());
    }

    // Typeahead on the closed trigger: "b" chooses Banana, then Blueberry on the next press.
    {
        let ui = FruitForm::new().expect("create the form");
        ui.invoke_focus_fruit();
        wire_nav(&ui, &ui.global::<SelectNav>());
        press_key(&ui, 'b');
        assert_eq!(ui.get_value().as_str(), "banana");
        press_key(&ui, 'b');
        assert_eq!(ui.get_value().as_str(), "blueberry");
        assert_eq!(ui.get_changes(), 2);
    }

    // Typeahead while open moves the highlight.
    {
        let ui = FruitForm::new().expect("create the form");
        ui.invoke_focus_fruit();
        wire_nav(&ui, &ui.global::<SelectNav>());
        press_key(&ui, ENTER);
        assert_eq!(
            ui.get_highlight(),
            0,
            "nothing chosen: the highlight starts on Apple"
        );
        press_key(&ui, 'c');
        assert_eq!(
            ui.get_highlight(),
            0,
            "Cherry is disabled, so typeahead finds no match and the highlight stays"
        );
        press_key(&ui, 'b');
        assert_eq!(ui.get_highlight(), 2, "Banana");
    }

    // A disabled Select does not open and does not change.
    {
        let ui = LockedForm::new().expect("create the form");
        wire_nav(&ui, &ui.global::<SelectNav>());
        ui.invoke_click_locked();
        assert!(!ui.get_menu_open(), "a disabled Select does not open");
        press_key(&ui, ENTER);
        assert!(!ui.get_menu_open());
        assert_eq!(ui.get_changes(), 0);
    }

    // A 200-option Select: the highlight can reach an item far below the visible rows, and the list scrolls to it.
    {
        let ui = LongForm::new().expect("create the form");
        ui.invoke_focus_long();
        wire_nav(&ui, &ui.global::<SelectNav>());
        let options: Vec<SelectOption> = (0..200)
            .map(|i| SelectOption {
                value: format!("item-{i}").into(),
                label: format!("Item {i}").into(),
                icon: Default::default(),
                badge: Default::default(),
                disabled: false,
                group: Default::default(),
            })
            .collect();
        ui.set_long_options(ModelRc::new(VecModel::from(options)));
        press_key(&ui, ENTER);
        assert!(ui.get_menu_open());
        assert_eq!(
            ui.get_highlight(),
            150,
            "the open menu highlights the chosen item"
        );
        press_special(&ui, Key::End);
        assert_eq!(ui.get_highlight(), 199, "End goes to the last option");
        press_special(&ui, Key::Home);
        assert_eq!(ui.get_highlight(), 0, "Home goes to the first option");
        press_special(&ui, DOWN);
        assert_eq!(ui.get_highlight(), 1);
        press_key(&ui, SPACE);
        assert!(!ui.get_menu_open());
        assert_eq!(
            ui.get_value().as_str(),
            "item-1",
            "Space chooses the highlighted option"
        );
    }
}
