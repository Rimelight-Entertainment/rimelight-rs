//! P0.17 batch 4 tests: DescriptionList, Stat, Timeline and ActivityFeed. Everything runs in one test function, because the
//! testing backend can be installed once per process (as in `tier2_b3.rs`).
//!
//! The form draws all four components and shows them. Only the ActivityFeed handles input: its "Load more" button is
//! reached by Tab and activated by Enter and Space (real key events through `Window::dispatch_event`), and by a click
//! (a test hook that fires the feed's own `load-more`, as its button does). The other three are display only and have no keys or callbacks to test.

use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle as _, ModelRc, SharedString, VecModel};

pub use rimelight_ui_test_slint::tier3_b4::*;

fn pair(term: &str, value: &str) -> DescriptionItem {
    DescriptionItem {
        term: SharedString::from(term),
        value: SharedString::from(value),
    }
}

fn event(when: &str, title: &str, body: &str, tone: BadgeTone) -> TimelineEvent {
    TimelineEvent {
        when: SharedString::from(when),
        title: SharedString::from(title),
        body: SharedString::from(body),
        tone,
    }
}

fn activity(actor: &str, initials: &str, action: &str, when: &str) -> ActivityItem {
    ActivityItem {
        actor: SharedString::from(actor),
        initials: SharedString::from(initials),
        action: SharedString::from(action),
        when: SharedString::from(when),
    }
}

fn press(ui: &B4Form, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

pub fn tier3_batch4_components_draw_and_load_more() {
    crate::support::init();
    let ui = B4Form::new().expect("create the form");

    // Five pairs: in two columns the left one takes the first three.
    ui.set_pairs(ModelRc::new(VecModel::from(vec![
        pair("Owner", "Ana Ruiz"),
        pair("Status", "Open"),
        pair("Due", "14 Nov 2026"),
        pair("Project", "Rimelight Space"),
        pair("Labels", "ui, tier 3"),
    ])));
    ui.set_events(ModelRc::new(VecModel::from(vec![
        event(
            "11 Nov, 09:40",
            "Build passed",
            "Release candidate 3 built.",
            BadgeTone::Success,
        ),
        event(
            "10 Nov, 17:05",
            "Review requested",
            "Waiting on design sign-off.",
            BadgeTone::Warning,
        ),
        event("9 Nov, 08:12", "Project created", "", BadgeTone::Neutral),
    ])));
    ui.set_feed(ModelRc::new(VecModel::from(vec![
        activity("Ana Ruiz", "AR", "moved DOC-12 to Done", "2 min ago"),
        activity("Sam Okafor", "SO", "commented on DOC-9", "1 h ago"),
        activity("Lin Chen", "LC", "created DOC-14", "Yesterday"),
    ])));
    ui.show().expect("show the form");

    // Drawn: the feed's button is shown while there is a next page.
    assert!(ui.invoke_button_shown(), "the button is drawn");

    // Keyboard: Tab reaches the button (the only Tab stop on the form); Enter and Space each ask for the next page.
    press(&ui, Key::Tab);
    press(&ui, Key::Return);
    assert_eq!(ui.get_loads(), 1, "Tab, then Enter, calls load-more once");
    press(&ui, Key::Space);
    assert_eq!(ui.get_loads(), 2, "Space calls load-more");

    // Pointer: a click calls load-more.
    ui.invoke_click_load_more();
    assert_eq!(ui.get_loads(), 3, "a click calls load-more");

    // The caller ends paging: the button is removed and no longer a target.
    ui.set_more(false);
    assert!(
        !ui.invoke_button_shown(),
        "the button goes when has-more is false"
    );

    // One column: the same items draw without the split (no callback is involved).
    ui.set_columns(1);
    ui.set_more(true);
    assert!(ui.invoke_button_shown(), "the button comes back");
    assert_eq!(ui.get_loads(), 3);
}
