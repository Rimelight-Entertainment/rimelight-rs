//! The `ToastSink` written by hand, with no macro. Compiled only with `--features manual-trait`.
//! It does the same work as the expansion of `spike_kit::wire_toasts!`.

use slint::{ComponentHandle as _, ModelRc, VecModel};
use spike_kit::{Toast, ToastSink};

use crate::{AppWindow, ToastEntry, ToastState};

pub struct ManualSink {
    window: slint::Weak<AppWindow>,
}

impl ManualSink {
    pub fn new(window: &AppWindow) -> Self {
        Self {
            window: window.as_weak(),
        }
    }
}

impl ToastSink for ManualSink {
    fn set_entries(&self, entries: Vec<Toast>) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let rows: Vec<ToastEntry> = entries
            .into_iter()
            .map(|toast| ToastEntry {
                id: toast.id,
                text: toast.text.into(),
                tone: toast.tone,
            })
            .collect();
        ui.global::<ToastState>()
            .set_entries(ModelRc::new(VecModel::from(rows)));
    }

    fn on_dismiss(&self, f: Box<dyn Fn(i32)>) {
        if let Some(ui) = self.window.upgrade() {
            ui.global::<ToastState>().on_dismiss(move |id| f(id));
        }
    }
}
