//! Shared setup for the UI tests. Every test calls [`init`] first.
//!
//! Slint allows one platform per process, bound to the thread that creates it. The `ui` binary runs all tests on the
//! main thread (see main.rs), so one `Once` covers them all. The testing backend uses mock time and the software
//! renderer, so tests can also render frames with `Window::take_snapshot`. No window is opened.

use std::sync::Once;

use i_slint_backend_testing::testing_backend::{TestingBackend, TestingBackendOptions};

static INIT: Once = Once::new();

/// Installs the headless testing backend once per process.
pub fn init() {
    INIT.call_once(|| {
        slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
            mock_time: true,
            threading: true,
            renderer_name: Some("software".into()),
        })))
        .expect("install the headless testing platform");
    });
}
