//! Rail badge text (OQ3). The rail draws a count on each work-queue area. Plain Rust, always compiled, so its tests run
//! without a window. The app builds the badge rows from its own areas (the rail's counts); this is only the text.

/// The text a rail badge shows for `count`: the number itself, or "99+" when it is above 99.
pub fn badge_text(count: u32) -> String {
    if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_and_99_show_the_number() {
        assert_eq!(badge_text(0), "0");
        assert_eq!(badge_text(99), "99");
    }

    #[test]
    fn counts_of_100_and_above_show_99_plus() {
        assert_eq!(badge_text(100), "99+");
        assert_eq!(badge_text(1000), "99+");
    }
}
