//! Plain-Rust toast queue. It owns the toast list and the auto-dismiss timers, and it pushes the list to the
//! app's Slint window through a [`ToastSink`]. This crate never names a type generated from markup.
//!
//! Two ways to connect a queue to an app (both are in the spike):
//!
//! 1. **Trait, by hand.** The app writes its own `impl ToastSink` (see `spike_app/src/manual.rs`) and calls
//!    `queue.attach(sink)`. The sink converts [`Toast`] to the app's generated row struct and sets the global.
//! 2. **Macro.** `spike_kit::wire_toasts!(AppWindow, ToastState, ToastEntry);` expands, inside the app crate, to a
//!    `WindowToastSink` newtype around `slint::Weak<AppWindow>` with the same `impl ToastSink`. The app then calls
//!    `queue.attach(WindowToastSink::new(&ui))`. The expansion names `::slint` paths, so the app crate must depend on
//!    `slint` directly.
//!
//! All of this runs on the UI thread: the queue is `Rc`-based, and `slint::Timer` fires on the event loop.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

/// One toast as the queue holds it. Plain data: no Slint types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub id: i32,
    pub text: String,
    /// 0 is neutral. The app's markup decides what other values look like.
    pub tone: i32,
}

/// Where the queue sends its list. The app implements it, by hand or with [`wire_toasts!`].
pub trait ToastSink {
    /// Replaces the whole list shown by the app, oldest first.
    fn set_entries(&self, entries: Vec<Toast>);
    /// Registers the handler the app calls when the user dismisses a toast by id.
    fn on_dismiss(&self, f: Box<dyn Fn(i32)>);
}

#[derive(Default)]
struct State {
    next_id: i32,
    toasts: Vec<Toast>,
    /// One single-shot timer per toast with a time limit. Dropping a timer stops it.
    timers: HashMap<i32, slint::Timer>,
    sink: Option<Rc<dyn ToastSink>>,
}

/// The toast list and its timers. Clone it freely: clones share the same state.
#[derive(Clone, Default)]
pub struct ToastQueue {
    state: Rc<RefCell<State>>,
}

impl ToastQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Connects the queue to the app's window and pushes the current list to it.
    ///
    /// The sink's dismiss handler holds only a weak reference to the queue, so the queue and the sink do not keep
    /// each other alive.
    pub fn attach(&self, sink: impl ToastSink + 'static) {
        let sink: Rc<dyn ToastSink> = Rc::new(sink);
        let weak = Rc::downgrade(&self.state);
        sink.on_dismiss(Box::new(move |id| {
            if let Some(state) = weak.upgrade() {
                ToastQueue { state }.dismiss(id);
            }
        }));
        self.state.borrow_mut().sink = Some(sink);
        self.publish();
    }

    /// Adds a toast and returns its id. A `ttl_ms` of 0 keeps it until it is dismissed.
    pub fn push(&self, text: impl Into<String>, tone: i32, ttl_ms: u64) -> i32 {
        let id = {
            let mut state = self.state.borrow_mut();
            state.next_id += 1;
            let id = state.next_id;
            state.toasts.push(Toast {
                id,
                text: text.into(),
                tone,
            });
            id
        };
        if ttl_ms > 0 {
            let timer = slint::Timer::default();
            let weak = Rc::downgrade(&self.state);
            timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(ttl_ms),
                move || {
                    if let Some(state) = weak.upgrade() {
                        ToastQueue { state }.dismiss(id);
                    }
                },
            );
            self.state.borrow_mut().timers.insert(id, timer);
        }
        self.publish();
        id
    }

    /// Removes a toast and its timer. Unknown ids are ignored.
    pub fn dismiss(&self, id: i32) {
        let removed = {
            let mut state = self.state.borrow_mut();
            let before = state.toasts.len();
            state.toasts.retain(|toast| toast.id != id);
            state.timers.remove(&id);
            state.toasts.len() != before
        };
        if removed {
            self.publish();
        }
    }

    /// The current list, oldest first.
    pub fn toasts(&self) -> Vec<Toast> {
        self.state.borrow().toasts.clone()
    }

    fn publish(&self) {
        let (sink, toasts) = {
            let state = self.state.borrow();
            (state.sink.clone(), state.toasts.clone())
        };
        if let Some(sink) = sink {
            sink.set_entries(toasts);
        }
    }
}

/// Generates, in the calling crate, a `WindowToastSink` that implements [`ToastSink`] for the app's window.
///
/// - `$window`: the app's window type, for example `AppWindow` (a `ty`).
/// - `$state`: the window's global holding the list and the dismiss callback, for example `ToastState`.
/// - `$entry`: the generated row struct with fields `id: i32`, `text: SharedString`, `tone: i32`, for example
///   `ToastEntry`.
///
/// Invoke it where those types are in scope (after `slint::include_modules!()`).
#[macro_export]
macro_rules! wire_toasts {
    ($window:ty, $state:ident, $entry:ident) => {
        /// The app's `ToastSink`, generated by `spike_kit::wire_toasts!`. It holds a weak handle, so it does not keep
        /// the window alive.
        pub struct WindowToastSink(::slint::Weak<$window>);

        impl WindowToastSink {
            pub fn new(window: &$window) -> Self {
                use ::slint::ComponentHandle as _;
                Self(window.as_weak())
            }
        }

        impl $crate::ToastSink for WindowToastSink {
            fn set_entries(&self, entries: ::std::vec::Vec<$crate::Toast>) {
                use ::slint::ComponentHandle as _;
                let Some(ui) = self.0.upgrade() else {
                    return;
                };
                let rows: ::std::vec::Vec<$entry> = entries
                    .into_iter()
                    .map(|toast| $entry {
                        id: toast.id,
                        text: toast.text.into(),
                        tone: toast.tone,
                    })
                    .collect();
                ui.global::<$state>()
                    .set_entries(::slint::ModelRc::new(::slint::VecModel::from(rows)));
            }

            fn on_dismiss(&self, f: ::std::boxed::Box<dyn ::std::ops::Fn(i32)>) {
                use ::slint::ComponentHandle as _;
                if let Some(ui) = self.0.upgrade() {
                    ui.global::<$state>().on_dismiss(move |id| f(id));
                }
            }
        }
    };
}
