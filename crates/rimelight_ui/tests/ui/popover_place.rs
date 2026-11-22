//! Diagnostic: an upward-opening Popover (bottom-limit set) read after its first open. Prints the measured height,
//! whether it opens above, and the popup's y, so a placement bug shows up as numbers.

pub use rimelight_ui_test_slint::popover_place::*;

pub fn popover_upward_placement_diagnostic() {
    crate::support::init();
    let form = PlaceForm::new().expect("place form builds");
    form.invoke_open_pop();
    for _ in 0..4 {
        slint::platform::update_timers_and_animations();
    }
    eprintln!(
        "DIAG open={} above={} eff={} pop-y={}",
        form.get_is_open(),
        form.get_above(),
        form.get_eff(),
        form.get_pop_y()
    );
    for _ in 0..4 {
        slint::platform::update_timers_and_animations();
    }
    eprintln!(
        "DIAG2 open={} above={} eff={} pop-y={} panel-h={} content-h={}",
        form.get_is_open(),
        form.get_above(),
        form.get_eff(),
        form.get_pop_y(),
        form.get_panel_h(),
        form.get_content_h()
    );
}
