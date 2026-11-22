//! S8 Toast interaction tests, driven by real key and pointer events through
//! `Window::dispatch_event`, the form's `click-*` hooks and mock time, as in `dialog.rs`. The queue is the plain
//! Rust `ToastQueue` (`rimelight_ui_kit::toast`): it owns the countdown, and the test ticks it with the mock clock, the way the
//! app's one timer does. The window's model is synced from the queue after every change. One test function, because the
//! testing backend is installed once per process.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use i_slint_backend_testing::mock_elapsed_time;
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle as _, LogicalPosition, ModelRc, SharedString, VecModel};

use rimelight_ui_kit::toast::{Dismiss, Record, Toast, ToastQueue, Tone};

pub use rimelight_ui_test_slint::toast::*;

const TICK_MS: u32 = 100;

struct Harness {
    ui: ToastForm,
    queue: Rc<RefCell<ToastQueue>>,
    model: Rc<VecModel<ToastEntry>>,
}

impl Harness {
    /// Shows a toast, re-syncs the window and returns its id.
    fn show(&self, toast: Toast) -> i32 {
        let id = self.queue.borrow_mut().show(toast);
        self.sync();
        id
    }

    fn sync(&self) {
        sync(&self.queue, &self.model);
    }

    fn is_closing(&self, id: i32) -> bool {
        self.queue
            .borrow()
            .visible()
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.closing)
            .unwrap_or(false)
    }

    fn present(&self, id: i32) -> bool {
        self.queue.borrow().contains(id)
    }

    /// Advances mock time in 100 ms steps. Each step is one frame and one tick of the queue's countdown, as the app's timer
    /// does it.
    fn advance(&self, ms: u64) {
        for _ in 0..(ms / TICK_MS as u64) {
            mock_elapsed_time(Duration::from_millis(TICK_MS as u64));
            // A frame: the window lays out and draws, which starts the exit timers of toasts that began leaving.
            let _ = self.ui.window().take_snapshot();
            slint::platform::update_timers_and_animations();
            self.queue.borrow_mut().tick(TICK_MS);
            self.sync();
        }
    }
}

/// Whether a toast has started leaving (its countdown ran out, or it was closed), or is already gone.
fn left(h: &Harness, id: i32) -> bool {
    h.is_closing(id) || !h.present(id)
}

fn press_special(ui: &ToastForm, key: Key) {
    let text: SharedString = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}

fn move_pointer(ui: &ToastForm, at: LogicalPosition) {
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position: at });
}

/// Copies the queue's visible toasts (newest first) into the window's model. The queue is not borrowed while the timers
/// run: an exit timer that fires calls back into the queue.
fn sync(queue: &RefCell<ToastQueue>, model: &VecModel<ToastEntry>) {
    let rows: Vec<ToastEntry> = queue.borrow().visible().into_iter().map(entry_of).collect();
    rimelight_ui_kit::toast::sync_model(model, &rows, |row| row.id);
    // Timers started by a change made outside an event start on the next pass.
    slint::platform::update_timers_and_animations();
}

fn entry_of(record: &Record) -> ToastEntry {
    ToastEntry {
        id: record.id,
        tone: match record.tone {
            Tone::Neutral => ToastTone::Neutral,
            Tone::Success => ToastTone::Success,
            Tone::Warning => ToastTone::Warning,
            Tone::Danger => ToastTone::Danger,
            Tone::Info => ToastTone::Info,
        },
        title: record.title.clone().into(),
        description: record.description.clone().into(),
        action_label: record.action_label.clone().unwrap_or_default().into(),
        close_label: record.close_label.clone().into(),
        closing: record.closing,
    }
}

pub fn toast_show_focus_actions_timers_and_keyboard() {
    crate::support::init();
    let ui = ToastForm::new().expect("create the form");
    ui.show().expect("show the form");

    let queue = Rc::new(RefCell::new(ToastQueue::new()));
    // The model is shared with the window; the wiring below re-syncs it after each callback.
    let model: Rc<VecModel<ToastEntry>> = Rc::new(VecModel::default());
    ui.set_toasts(ModelRc::from(model.clone()));
    {
        let (q, m) = (queue.clone(), model.clone());
        ui.on_action_clicked(move |id| {
            q.borrow_mut().action(id);
            sync(&q, &m);
        });
    }
    {
        let (q, m) = (queue.clone(), model.clone());
        ui.on_close_requested(move |id| {
            q.borrow_mut().dismiss(id);
            sync(&q, &m);
        });
    }
    {
        let (q, m) = (queue.clone(), model.clone());
        ui.on_exit_done(move |id| {
            q.borrow_mut().remove(id);
            sync(&q, &m);
        });
    }
    {
        let q = queue.clone();
        ui.on_paused_changed(move |id, paused| q.borrow_mut().set_paused(id, paused));
    }
    let h = Harness { ui, queue, model };

    // 1. Showing a toast does not change the focused element.
    for _ in 0..3 {
        if h.ui.get_behind_focused() {
            break;
        }
        press_special(&h.ui, Key::Tab);
    }
    assert!(h.ui.get_behind_focused(), "Tab gives the page button focus");
    let mut undo = Toast::new(Tone::Success, "Saved", "Dismiss saved");
    undo.description = "Your changes are in.".into();
    let counter = Rc::new(RefCell::new(0));
    let c = counter.clone();
    undo.action = Some(("Undo".into(), Box::new(move || *c.borrow_mut() += 1)));
    let saved = h.show(undo);
    assert!(
        h.ui.get_behind_focused(),
        "showing a toast keeps the focus on the page"
    );
    assert_eq!(
        h.ui.get_newest_action().as_str(),
        "Undo",
        "the action is drawn"
    );

    // 2. The action fires once and the toast closes (after its exit fade).
    h.ui.invoke_click_action(saved);
    h.ui.invoke_click_action(saved);
    assert_eq!(*counter.borrow(), 1, "the action fires once");
    assert!(h.is_closing(saved), "the action closes the toast");
    h.advance(400);
    assert!(
        !h.present(saved),
        "the toast is removed after its exit fade"
    );
    assert_eq!(h.ui.get_toast_count(), 0, "the title leaves the window");

    // 3. The close control removes the toast.
    let plain = h.show(Toast::new(Tone::Info, "Sync started", "Dismiss sync"));
    h.ui.invoke_click_close(plain);
    h.advance(400);
    assert!(!h.present(plain), "the close control removes the toast");

    // 4. dismiss_all clears everything.
    h.show(Toast::new(Tone::Neutral, "One", "Close one"));
    h.show(Toast::new(Tone::Info, "Two", "Close two"));
    h.queue.borrow_mut().dismiss_all();
    h.sync();
    h.advance(400);
    let remaining: Vec<i32> = h.queue.borrow().visible().iter().map(|r| r.id).collect();
    assert!(
        remaining.is_empty(),
        "dismiss_all clears the stack: {remaining:?}"
    );

    // 5. Auto-dismiss: neutral at 5 s, warning at 8 s (not 5 s), danger never. The pointer is moved off the stack first: the
    // last mock click left it over a close control, and a pointer over a toast pauses its countdown.
    move_pointer(&h.ui, LogicalPosition::new(-5.0, -5.0));
    let neutral = h.show(Toast::new(Tone::Neutral, "Neutral", "Close neutral"));
    h.advance(4900);
    assert!(
        !left(&h, neutral),
        "a neutral toast is still shown at 4.9 s"
    );
    h.advance(100);
    assert!(left(&h, neutral), "a neutral toast leaves at 5 s");
    h.advance(400);
    assert!(!h.present(neutral));

    let warning = h.show(Toast::new(Tone::Warning, "Careful", "Close warning"));
    h.advance(5000);
    assert!(!left(&h, warning), "a warning toast is still shown at 5 s");
    h.advance(2900);
    assert!(
        !left(&h, warning),
        "a warning toast is still shown at 7.9 s"
    );
    h.advance(100);
    assert!(left(&h, warning), "a warning toast leaves at 8 s");
    h.advance(400);

    let danger = h.show(Toast::new(Tone::Danger, "Failed", "Close danger"));
    h.advance(60_000);
    assert!(!h.is_closing(danger), "a danger toast stays after 60 s");
    h.queue.borrow_mut().dismiss(danger);
    h.sync();
    h.advance(400);

    // 6. Hover pauses the countdown: the pointer over the toast stops it, leaving resumes it.
    let hovered = h.show(Toast::new(Tone::Neutral, "Hover me", "Close hover"));
    h.advance(3000);
    // The newest toast is the only one, at the stack's top left: a point inside its padding is over its body.
    let (x, y) = (h.ui.get_stack_x(), h.ui.get_stack_y());
    move_pointer(&h.ui, LogicalPosition::new(x + 4.0, y + 4.0));
    h.advance(10_000);
    assert!(!h.is_closing(hovered), "a hovered toast does not leave");
    move_pointer(&h.ui, LogicalPosition::new(-5.0, -5.0));
    h.advance(1500);
    assert!(
        !left(&h, hovered),
        "the countdown resumes from where it stopped"
    );
    h.advance(400);
    assert!(!left(&h, hovered), "still 100 ms to go");
    h.advance(100);
    assert!(
        left(&h, hovered),
        "the toast leaves after its remaining time"
    );
    h.advance(400);

    // 7. F6 moves focus into the newest toast, and Tab walks its buttons.
    let fired = Rc::new(RefCell::new(0));
    let f = fired.clone();
    let mut keyed = Toast::new(Tone::Danger, "Upload failed", "Dismiss upload");
    keyed.dismiss = Dismiss::Never;
    keyed.action = Some(("Retry".into(), Box::new(move || *f.borrow_mut() += 1)));
    let keyed_id = h.show(keyed);
    press_special(&h.ui, Key::F6);
    assert!(!h.ui.get_behind_focused(), "F6 moves focus out of the page");
    press_special(&h.ui, Key::Return);
    assert_eq!(
        *fired.borrow(),
        1,
        "Enter on the focused action runs it once"
    );
    assert!(h.is_closing(keyed_id));
    h.advance(400);
    assert!(!h.present(keyed_id));

    // Shift+Tab from the action reaches the close control of the same toast (the close control comes first in the markup,
    // in the title row); Enter there closes it, and the action does not run.
    let mut second_toast = Toast::new(Tone::Warning, "Second", "Dismiss second");
    second_toast.action = Some(("Keep".into(), Box::new(|| {})));
    let second = h.show(second_toast);
    press_special(&h.ui, Key::F6);
    press_special(&h.ui, Key::Backtab);
    press_special(&h.ui, Key::Return);
    assert!(
        h.is_closing(second),
        "Tab then Enter reaches the close control"
    );
    assert_eq!(
        *fired.borrow(),
        1,
        "the close control does not run the action"
    );
    h.advance(400);
    assert!(!h.present(second));
}
