// Headless test: the testing backend, no window on screen. Push a toast through the queue, read the global,
// call dismiss through the global, and read it again.

use i_slint_backend_testing::init_integration_test_with_mock_time;
use slint::{ComponentHandle as _, Model as _};
use spike_app::{AppWindow, ToastState};
use spike_kit::{ToastQueue, ToastSink};

#[cfg(not(feature = "manual-trait"))]
fn sink(ui: &AppWindow) -> impl ToastSink + 'static {
    spike_app::WindowToastSink::new(ui)
}

#[cfg(feature = "manual-trait")]
fn sink(ui: &AppWindow) -> impl ToastSink + 'static {
    spike_app::manual::ManualSink::new(ui)
}

#[test]
fn push_shows_a_row_and_dismiss_removes_it() {
    init_integration_test_with_mock_time();
    let ui = AppWindow::new().expect("create AppWindow");
    let queue = ToastQueue::new();
    queue.attach(sink(&ui));

    queue.push("Saved", 0, 5000);

    let state = ui.global::<ToastState>();
    assert_eq!(state.get_entries().row_count(), 1);
    let row = state.get_entries().row_data(0).expect("one row");
    assert_eq!(row.text.as_str(), "Saved");

    state.invoke_dismiss(row.id);
    assert_eq!(state.get_entries().row_count(), 0);
    assert!(queue.toasts().is_empty());
}
