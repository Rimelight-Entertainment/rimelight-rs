//! The one UI test binary for `rimelight_ui`. Each test file is a module here, so the crate links Slint once instead of
//! once per file. Add new UI tests as `tests/ui/<name>.rs` (never a new top-level `tests/*.rs`), then list them in `TESTS`.
//!
//! No libtest harness (`harness = false` in Cargo.toml). Slint's platform is bound to the thread that first creates it,
//! and libtest runs every test on a new thread, so the tests run one after another on the main thread. An optional first
//! argument runs only the tests whose name contains it.

use std::io::Write as _;
use std::process::ExitCode;

mod actions;
mod banner;
mod choice;
mod color_picker;
mod combobox;
mod context_menu;
mod dialog;
mod display;
mod feedback;
mod field;
mod keyboard;
mod menu;
mod number_input;
mod popover;
mod popover_place;
mod select;
mod sheet;
mod support;
mod text_fields;
mod textarea;
mod tier2_b1;
mod tier2_b2;
mod tier2_b3;
mod tier3_b4;
mod tier3_b5;
mod toast;
mod tooltip;

/// Every UI test, in run order.
const TESTS: &[(&str, fn())] = &[
    (
        "actions::action_components_click_keys_and_labels",
        actions::action_components_click_keys_and_labels,
    ),
    (
        "banner::banner_action_and_dismiss_fire_once_each",
        banner::banner_action_and_dismiss_fire_once_each,
    ),
    (
        "choice::choice_controls_with_real_key_and_pointer_events",
        choice::choice_controls_with_real_key_and_pointer_events,
    ),
    (
        "color_picker::color_picker_hex_callback_is_wired_to_the_parser",
        color_picker::color_picker_hex_callback_is_wired_to_the_parser,
    ),
    (
        "combobox::combobox_with_real_key_and_pointer_events",
        combobox::combobox_with_real_key_and_pointer_events,
    ),
    (
        "context_menu::context_menu_with_real_key_and_pointer_events",
        context_menu::context_menu_with_real_key_and_pointer_events,
    ),
    (
        "dialog::dialog_focus_dismiss_and_confirm_with_real_events",
        dialog::dialog_focus_dismiss_and_confirm_with_real_events,
    ),
    (
        "display::display_components_cap_remove_and_split",
        display::display_components_cap_remove_and_split,
    ),
    (
        "feedback::feedback_actions_fire_once_per_activation",
        feedback::feedback_actions_fire_once_per_activation,
    ),
    (
        "field::field_error_and_form_tab_order",
        field::field_error_and_form_tab_order,
    ),
    (
        "keyboard::pointer_focus_typing_tab_and_activation",
        keyboard::pointer_focus_typing_tab_and_activation,
    ),
    (
        "menu::menu_with_real_key_and_pointer_events",
        menu::menu_with_real_key_and_pointer_events,
    ),
    (
        "number_input::number_input_behaviour_with_real_events",
        number_input::number_input_behaviour_with_real_events,
    ),
    (
        "popover::popover_with_real_key_and_pointer_events",
        popover::popover_with_real_key_and_pointer_events,
    ),
    (
        "popover_place::popover_upward_placement_diagnostic",
        popover_place::popover_upward_placement_diagnostic,
    ),
    (
        "select::select_with_real_key_and_pointer_events",
        select::select_with_real_key_and_pointer_events,
    ),
    (
        "sheet::sheet_focus_dismiss_placement_and_motion",
        sheet::sheet_focus_dismiss_placement_and_motion,
    ),
    (
        "text_fields::input_behaviour_with_real_key_and_pointer_events",
        text_fields::input_behaviour_with_real_key_and_pointer_events,
    ),
    (
        "textarea::textarea_behaviour_with_real_key_and_pointer_events",
        textarea::textarea_behaviour_with_real_key_and_pointer_events,
    ),
    (
        "tier2_b1::tier2_batch1_components_draw_and_breadcrumbs_navigate",
        tier2_b1::tier2_batch1_components_draw_and_breadcrumbs_navigate,
    ),
    (
        "tier2_b2::tier2_batch2_components_keys_and_callbacks",
        tier2_b2::tier2_batch2_components_keys_and_callbacks,
    ),
    (
        "tier2_b3::visible_rows_hide_children_of_collapsed_nodes",
        tier2_b3::visible_rows_hide_children_of_collapsed_nodes,
    ),
    (
        "tier2_b3::tier2_batch3_components_keys_and_callbacks",
        tier2_b3::tier2_batch3_components_keys_and_callbacks,
    ),
    (
        "tier3_b4::tier3_batch4_components_draw_and_load_more",
        tier3_b4::tier3_batch4_components_draw_and_load_more,
    ),
    (
        "tier3_b5::tier3_batch5_list_and_table",
        tier3_b5::tier3_batch5_list_and_table,
    ),
    (
        "toast::toast_show_focus_actions_timers_and_keyboard",
        toast::toast_show_focus_actions_timers_and_keyboard,
    ),
    (
        "tooltip::tooltip_shows_on_focus_and_hover_and_hides_on_escape_blur_and_leave",
        tooltip::tooltip_shows_on_focus_and_hover_and_hides_on_escape_blur_and_leave,
    ),
];

fn main() -> ExitCode {
    // The first argument that is not a flag. Cargo or the caller may add flags, which must not filter out every test.
    let filter = std::env::args().skip(1).find(|arg| !arg.starts_with('-'));
    let mut passed = 0;
    let mut failed = Vec::new();
    for (name, test) in TESTS {
        if filter.as_deref().is_some_and(|f| !name.contains(f)) {
            continue;
        }
        print!("test {name} ... ");
        let _ = std::io::stdout().flush();
        match std::panic::catch_unwind(*test) {
            Ok(()) => {
                println!("ok");
                passed += 1;
            }
            Err(_) => {
                println!("FAILED");
                failed.push(*name);
            }
        }
    }
    println!("\nui: {passed} passed; {} failed", failed.len());
    for name in &failed {
        println!("  failed: {name}");
    }
    if failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
