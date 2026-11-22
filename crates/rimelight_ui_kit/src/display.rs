//! Plain-Rust rules of the S4 display and feedback components: the badge cap, kbd combos, progress clamping, the avatar
//! stack, the initials of an avatar and reduced motion. No Slint types here, so the tests are plain unit tests.
//!
//! Slint has no string split and no list join in 1.18.1, so the key split of [`parse_combo`] is done here, and the
//! markup is given the results of these functions.

/// The text a count badge shows: the number, or `max+` above `max`.
pub fn badge_label(count: u32, max: u32) -> String {
    if count > max {
        format!("{max}+")
    } else {
        count.to_string()
    }
}

/// Splits a combo such as `Ctrl+Shift+K` into its keys. Empty parts are dropped.
pub fn parse_combo(combo: &str) -> Vec<String> {
    combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// A progress value clamped to 0 through 1. NaN counts as 0, so a bad value never draws past the track.
pub fn clamp_progress(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// How an avatar stack splits `total` people into the faces it shows and the `+N` overflow count, when at most
/// `max_shown` faces fit. Returns `(shown, overflow)`. The overflow is 0 when everyone fits.
pub fn avatar_overflow(total: usize, max_shown: usize) -> (usize, usize) {
    if total <= max_shown {
        (total, 0)
    } else {
        // The last slot is the `+N` chip, so one fewer face is shown than fit.
        let shown = max_shown.saturating_sub(1);
        (shown, total - shown)
    }
}

/// The initials an avatar shows for a name: the first letter of the first two words, upper case. `?` when there is none
/// Slint cannot take a letter out of a string, so the caller passes this result.
pub fn initials(name: &str) -> String {
    let letters: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|word| word.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    if letters.is_empty() {
        "?".to_string()
    } else {
        letters
    }
}

/// The duration of a motion token in milliseconds. Reduced motion makes every animation instant (0 ms).
pub fn motion_duration_ms(base_ms: u32, reduced_motion: bool) -> u32 {
    if reduced_motion { 0 } else { base_ms }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_caps_at_max() {
        assert_eq!(badge_label(0, 99), "0");
        assert_eq!(badge_label(99, 99), "99");
        assert_eq!(badge_label(100, 99), "99+");
        assert_eq!(badge_label(1234, 999), "999+");
        assert_eq!(badge_label(999, 999), "999");
    }

    #[test]
    fn kbd_combos_split_on_plus() {
        assert_eq!(parse_combo("Ctrl+K"), vec!["Ctrl", "K"]);
        assert_eq!(parse_combo("Ctrl+Shift+P"), vec!["Ctrl", "Shift", "P"]);
        assert_eq!(parse_combo("Esc"), vec!["Esc"]);
        assert_eq!(parse_combo("Ctrl+"), vec!["Ctrl"]);
        assert_eq!(parse_combo(" Alt + Tab "), vec!["Alt", "Tab"]);
        assert!(parse_combo("").is_empty());
    }

    #[test]
    fn progress_is_clamped_to_the_track() {
        assert_eq!(clamp_progress(-0.5), 0.0);
        assert_eq!(clamp_progress(0.25), 0.25);
        assert_eq!(clamp_progress(1.7), 1.0);
        assert_eq!(clamp_progress(f32::NAN), 0.0);
    }

    #[test]
    fn avatar_stack_overflows_into_a_count() {
        assert_eq!(avatar_overflow(3, 4), (3, 0));
        assert_eq!(avatar_overflow(4, 4), (4, 0));
        // Five people, room for four: three faces and a "+2" chip.
        assert_eq!(avatar_overflow(5, 4), (3, 2));
        assert_eq!(avatar_overflow(12, 4), (3, 9));
        assert_eq!(avatar_overflow(0, 0), (0, 0));
    }

    #[test]
    fn initials_take_the_first_two_words() {
        assert_eq!(initials("Ana Souza"), "AS");
        assert_eq!(initials("eva"), "E");
        assert_eq!(initials("Ana Maria Souza"), "AM");
        assert_eq!(initials(""), "?");
    }

    #[test]
    fn reduced_motion_makes_durations_zero() {
        assert_eq!(motion_duration_ms(1200, true), 0);
        assert_eq!(motion_duration_ms(1200, false), 1200);
    }
}
