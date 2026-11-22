// Headless test: the testing backend, no window on screen. Creating CheckWindow runs the component tree, so a
// component that fails at runtime setup fails here as well as at compile time.

use i_slint_backend_testing::init_integration_test_with_mock_time;
use rimelight_ui_check::CheckWindow;

#[test]
fn check_window_instantiates_every_component() {
    init_integration_test_with_mock_time();
    let _window = CheckWindow::new().expect("create CheckWindow");
}
