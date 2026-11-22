//! Plain Rust: the queue holds no Slint types. `sync_model` is generic over the app's row type.
//! The queue tests are unit tests of this crate.
//!
//! The queue holds every toast that has not left yet, in arrival order. The stack shows the newest [`VISIBLE`] of them,
//! newest on top; the older ones wait in the queue and move onto the stack when a newer toast leaves.
//!
//! Timing lives here, not in the markup. Each toast's countdown is a number in the queue; the host calls
//! [`ToastQueue::tick`] from one timer and [`ToastQueue::set_paused`] when the markup reports hover or focus. The markup
//! only draws and runs the exit animation. The host calls [`ToastQueue::dismiss`] when the stack asks for a close, and
//! [`ToastQueue::remove`] when the exit animation is done.

use std::cell::RefCell;

/// The most toasts the stack shows at once. Older toasts wait in the queue.
pub const VISIBLE: usize = 5;

thread_local! {
    /// Toasts announced by pages, waiting for the app shell's timer to move them into the queue.
    static ANNOUNCED: RefCell<Vec<(Tone, String)>> = const { RefCell::new(Vec::new()) };
}

/// Asks for a toast from code that has no access to the queue (a page's result handler). The app shell's timer moves it
/// into the queue on its next tick, via [`take_announced`]. Call it on the UI thread.
pub fn announce(tone: Tone, title: impl Into<String>) {
    ANNOUNCED.with(|pending| pending.borrow_mut().push((tone, title.into())));
}

/// Takes every toast announced since the last call, oldest first.
pub fn take_announced() -> Vec<(Tone, String)> {
    ANNOUNCED.with(|pending| std::mem::take(&mut *pending.borrow_mut()))
}

/// The tone of a toast. The markup draws each tone from the status tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Success,
    Warning,
    Danger,
    Info,
}

/// When a toast dismisses by itself: the tone's default, never, or after a time in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dismiss {
    /// 5 s for neutral, success and info; 8 s for warning; never for danger.
    ToneDefault,
    /// Stays until closed.
    Never,
    /// Dismisses after this many milliseconds (paused while hovered or focused).
    After(u32),
}

impl Dismiss {
    /// The countdown a toast of `tone` starts with, in milliseconds. `None` means it stays until it is closed.
    pub fn countdown_ms(self, tone: Tone) -> Option<u32> {
        let ms = match self {
            Dismiss::ToneDefault => default_dismiss_ms(tone),
            Dismiss::Never => 0,
            Dismiss::After(ms) => ms,
        };
        (ms > 0).then_some(ms)
    }
}

/// One toast as the caller asks for it. `action` is an optional label; the callback given with it runs once when the
/// action is chosen, and the toast then closes.
pub struct Toast {
    pub tone: Tone,
    pub title: String,
    pub description: String,
    pub action: Option<(String, Box<dyn FnMut()>)>,
    pub close_label: String,
    pub dismiss: Dismiss,
}

impl Toast {
    /// A toast with a title, the tone's default dismissal, and no description or action.
    pub fn new(tone: Tone, title: impl Into<String>, close_label: impl Into<String>) -> Self {
        Self {
            tone,
            title: title.into(),
            description: String::new(),
            action: None,
            close_label: close_label.into(),
            dismiss: Dismiss::ToneDefault,
        }
    }
}

/// One toast in the queue: its id, what the markup shows, and whether it is leaving.
pub struct Record {
    pub id: i32,
    pub tone: Tone,
    pub title: String,
    pub description: String,
    pub action_label: Option<String>,
    pub close_label: String,
    pub dismiss: Dismiss,
    pub closing: bool,
    /// Milliseconds left before the toast starts leaving. `None` when it never dismisses by itself.
    pub remaining_ms: Option<u32>,
    /// Set by the markup while the pointer or keyboard focus is on the toast. A paused toast keeps its remaining time.
    pub paused: bool,
    action: Option<Box<dyn FnMut()>>,
}

/// The toasts that have not left yet, in arrival order.
#[derive(Default)]
pub struct ToastQueue {
    records: Vec<Record>,
    next_id: i32,
}

impl ToastQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a toast and returns its id. The id is never reused.
    pub fn show(&mut self, toast: Toast) -> i32 {
        self.next_id += 1;
        let id = self.next_id;
        let (action_label, action) = match toast.action {
            Some((label, callback)) => (Some(label), Some(callback)),
            None => (None, None),
        };
        self.records.push(Record {
            id,
            tone: toast.tone,
            title: toast.title,
            description: toast.description,
            action_label,
            close_label: toast.close_label,
            dismiss: toast.dismiss,
            closing: false,
            remaining_ms: toast.dismiss.countdown_ms(toast.tone),
            paused: false,
            action,
        });
        id
    }

    /// Sets whether a toast's countdown is paused. Called when the pointer or focus enters or leaves the toast.
    pub fn set_paused(&mut self, id: i32, paused: bool) {
        if let Some(record) = self.records.iter_mut().find(|r| r.id == id) {
            record.paused = paused;
        }
    }

    /// Counts the visible toasts down by `elapsed_ms`. A toast whose time runs out starts leaving, as if closed. Paused
    /// and leaving toasts do not count, and neither do the toasts waiting outside the stack: their countdown starts when
    /// they reach the stack. The host calls this from one timer, with the time since the last call.
    pub fn tick(&mut self, elapsed_ms: u32) {
        let start = self.records.len().saturating_sub(VISIBLE);
        for record in &mut self.records[start..] {
            if record.closing || record.paused {
                continue;
            }
            let Some(remaining) = record.remaining_ms.as_mut() else {
                continue;
            };
            *remaining = remaining.saturating_sub(elapsed_ms);
            if *remaining == 0 {
                record.closing = true;
            }
        }
    }

    /// Marks a toast as leaving. It stays in the queue until [`ToastQueue::remove`] (after its exit animation). Returns
    /// false when there is no such toast or it is already leaving.
    pub fn dismiss(&mut self, id: i32) -> bool {
        match self.records.iter_mut().find(|r| r.id == id) {
            Some(record) if !record.closing => {
                record.closing = true;
                true
            }
            _ => false,
        }
    }

    /// Marks every toast as leaving.
    pub fn dismiss_all(&mut self) {
        for record in &mut self.records {
            record.closing = true;
        }
    }

    /// Runs the action of a toast once and marks it leaving. A toast that is already leaving does nothing. Returns false
    /// when nothing ran.
    pub fn action(&mut self, id: i32) -> bool {
        let Some(record) = self.records.iter_mut().find(|r| r.id == id) else {
            return false;
        };
        if record.closing {
            return false;
        }
        if let Some(callback) = record.action.as_mut() {
            callback();
        }
        // The action is taken: it cannot run again, and the toast closes.
        record.action = None;
        record.closing = true;
        true
    }

    /// Removes a toast from the queue. Called when its exit animation is done.
    pub fn remove(&mut self, id: i32) {
        self.records.retain(|r| r.id != id);
    }

    /// Whether a toast is still in the queue (shown or waiting).
    pub fn contains(&self, id: i32) -> bool {
        self.records.iter().any(|r| r.id == id)
    }

    /// The toasts the stack shows, newest first: the last [`VISIBLE`] toasts in the queue, reversed.
    pub fn visible(&self) -> Vec<&Record> {
        let start = self.records.len().saturating_sub(VISIBLE);
        self.records[start..].iter().rev().collect()
    }

    /// The toasts waiting outside the stack, oldest first.
    pub fn queued(&self) -> Vec<&Record> {
        let end = self.records.len().saturating_sub(VISIBLE);
        self.records[..end].iter().collect()
    }
}

/// The default dismissal of a tone, in milliseconds. 0 means never.
pub fn default_dismiss_ms(tone: Tone) -> u32 {
    match tone {
        Tone::Warning => 8000,
        Tone::Danger => 0,
        Tone::Neutral | Tone::Success | Tone::Info => 5000,
    }
}

/// Brings a Slint model in line with `rows` (the visible toasts, newest first) by inserting, removing and updating single
/// rows. A whole-model reset would rebuild every toast and restart its countdown and exit fade, so rows that stay are kept.
/// `id` gives a row's id.
pub fn sync_model<T: Clone + PartialEq + 'static>(
    model: &slint::VecModel<T>,
    rows: &[T],
    id: impl Fn(&T) -> i32,
) {
    use slint::Model as _;
    // Remove the rows that are no longer shown, from the end so the indices stay valid.
    for index in (0..model.row_count()).rev() {
        let Some(row) = model.row_data(index) else {
            continue;
        };
        if !rows.iter().any(|want| id(want) == id(&row)) {
            model.remove(index);
        }
    }
    // Then walk the wanted rows in order: insert the new ones, update the changed ones.
    for (index, want) in rows.iter().enumerate() {
        match model.row_data(index) {
            Some(row) if id(&row) == id(want) => {
                if row != *want {
                    model.set_row_data(index, want.clone());
                }
            }
            _ => model.insert(index, want.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn toast(title: &str) -> Toast {
        Toast::new(Tone::Neutral, title, "Close")
    }

    #[test]
    fn newest_visible_on_top_and_older_queued() {
        let mut q = ToastQueue::new();
        for n in 0..7 {
            q.show(toast(&format!("t{n}")));
        }
        let visible: Vec<&str> = q.visible().iter().map(|r| r.title.as_str()).collect();
        assert_eq!(visible, ["t6", "t5", "t4", "t3", "t2"]);
        let queued: Vec<&str> = q.queued().iter().map(|r| r.title.as_str()).collect();
        assert_eq!(queued, ["t0", "t1"]);
    }

    #[test]
    fn leaving_toast_moves_the_oldest_queued_onto_the_stack() {
        let mut q = ToastQueue::new();
        let ids: Vec<i32> = (0..6).map(|n| q.show(toast(&format!("t{n}")))).collect();
        assert!(q.dismiss(ids[5]));
        q.remove(ids[5]);
        let visible: Vec<&str> = q.visible().iter().map(|r| r.title.as_str()).collect();
        assert_eq!(visible, ["t4", "t3", "t2", "t1", "t0"]);
        assert!(q.queued().is_empty());
    }

    #[test]
    fn ids_are_unique_and_dismiss_is_idempotent() {
        let mut q = ToastQueue::new();
        let a = q.show(toast("a"));
        let b = q.show(toast("b"));
        assert_ne!(a, b);
        assert!(q.dismiss(a));
        assert!(!q.dismiss(a));
        assert!(!q.dismiss(999));
        assert!(q.contains(a), "dismiss marks, remove deletes");
        q.remove(a);
        assert!(!q.contains(a));
    }

    #[test]
    fn action_runs_once_and_closes() {
        let runs = Rc::new(Cell::new(0));
        let counter = runs.clone();
        let mut q = ToastQueue::new();
        let mut t = toast("saved");
        t.action = Some((
            "Undo".into(),
            Box::new(move || counter.set(counter.get() + 1)),
        ));
        let id = q.show(t);
        assert!(q.action(id));
        assert!(!q.action(id));
        assert_eq!(runs.get(), 1);
        assert!(q.visible()[0].closing);
    }

    #[test]
    fn dismiss_all_marks_everything() {
        let mut q = ToastQueue::new();
        for n in 0..3 {
            q.show(toast(&format!("t{n}")));
        }
        q.dismiss_all();
        assert!(q.visible().iter().all(|r| r.closing));
    }

    #[test]
    fn announced_toasts_are_taken_once_in_order() {
        announce(Tone::Success, "Position created");
        announce(Tone::Success, "Team deleted");
        assert_eq!(
            take_announced(),
            vec![
                (Tone::Success, "Position created".to_string()),
                (Tone::Success, "Team deleted".to_string()),
            ]
        );
        assert!(take_announced().is_empty(), "a second take is empty");
    }

    #[test]
    fn countdown_follows_the_tone_and_the_setting() {
        assert_eq!(Dismiss::ToneDefault.countdown_ms(Tone::Neutral), Some(5000));
        assert_eq!(Dismiss::ToneDefault.countdown_ms(Tone::Warning), Some(8000));
        assert_eq!(Dismiss::ToneDefault.countdown_ms(Tone::Danger), None);
        assert_eq!(Dismiss::Never.countdown_ms(Tone::Neutral), None);
        assert_eq!(Dismiss::After(2500).countdown_ms(Tone::Danger), Some(2500));
        assert_eq!(default_dismiss_ms(Tone::Success), 5000);
        assert_eq!(default_dismiss_ms(Tone::Info), 5000);
    }

    fn timed(tone: Tone, title: &str) -> Toast {
        Toast::new(tone, title, "Close")
    }

    #[test]
    fn neutral_toast_leaves_at_five_seconds() {
        let mut q = ToastQueue::new();
        let id = q.show(timed(Tone::Neutral, "n"));
        q.tick(4900);
        assert!(!q.visible()[0].closing, "still shown at 4.9 s");
        q.tick(100);
        assert!(q.visible()[0].closing, "leaves at 5 s");
        assert!(q.contains(id), "leaving keeps it in the queue until remove");
    }

    #[test]
    fn warning_toast_is_not_removed_at_five_seconds() {
        let mut q = ToastQueue::new();
        q.show(timed(Tone::Warning, "w"));
        q.tick(5000);
        assert!(!q.visible()[0].closing, "still shown at 5 s");
        q.tick(2999);
        assert!(!q.visible()[0].closing, "still shown at 7.999 s");
        q.tick(1);
        assert!(q.visible()[0].closing, "leaves at 8 s");
    }

    #[test]
    fn danger_toast_never_leaves_by_itself() {
        let mut q = ToastQueue::new();
        q.show(timed(Tone::Danger, "d"));
        for _ in 0..600 {
            q.tick(100);
        }
        assert!(!q.visible()[0].closing, "a danger toast stays after 60 s");
    }

    #[test]
    fn paused_toast_keeps_its_remaining_time() {
        let mut q = ToastQueue::new();
        let id = q.show(timed(Tone::Neutral, "p"));
        q.tick(3000);
        q.set_paused(id, true);
        q.tick(10_000);
        assert!(!q.visible()[0].closing, "a paused toast does not leave");
        assert_eq!(q.visible()[0].remaining_ms, Some(2000));
        q.set_paused(id, false);
        q.tick(1900);
        assert!(!q.visible()[0].closing, "resumes from where it stopped");
        q.tick(100);
        assert!(q.visible()[0].closing, "leaves after the remaining time");
    }

    #[test]
    fn toast_shown_after_a_removal_still_counts_down() {
        let mut q = ToastQueue::new();
        let first = q.show(timed(Tone::Neutral, "first"));
        q.dismiss(first);
        q.remove(first);
        q.show(timed(Tone::Warning, "second"));
        q.tick(5000);
        assert!(!q.visible()[0].closing);
        q.tick(3000);
        assert!(
            q.visible()[0].closing,
            "the later toast leaves at its own 8 s"
        );
    }

    #[test]
    fn queued_toasts_do_not_count_until_they_reach_the_stack() {
        let mut q = ToastQueue::new();
        for n in 0..(VISIBLE + 1) {
            q.show(Toast::new(Tone::Info, format!("t{n}"), "Close"));
        }
        // The visible toasts are paused, so a tick here can only reach the queued one, and it must not count it.
        let visible_ids: Vec<i32> = q.visible().iter().map(|r| r.id).collect();
        for id in visible_ids {
            q.set_paused(id, true);
        }
        q.tick(5000);
        assert_eq!(
            q.queued()[0].remaining_ms,
            Some(5000),
            "a queued toast is not counted"
        );
        assert!(q.visible().iter().all(|r| !r.closing));
    }
}
