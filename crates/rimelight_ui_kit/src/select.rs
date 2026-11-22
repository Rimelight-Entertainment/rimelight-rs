//! Plain-Rust rules of the S6 Select and Combobox: the next enabled option for the arrow keys, typeahead, the group
//! headings, the Combobox filter and the index of a value. No Slint types here. The app expands [`wire_select!`] to get
//! the `SelectNav` wiring and the Slint-to-rules conversion, in the crate that generates the Slint types.

/// One option as the search sees it: its label, value and whether it is disabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub value: String,
    pub label: String,
    pub disabled: bool,
    pub group: String,
}

/// The next enabled option from `from` in direction `dir` (1 for down, -1 for up). Disabled options are skipped and the
/// search does not wrap: at either end it returns `from` unchanged. `from` is -1 when nothing is highlighted yet.
pub fn next_enabled(options: &[Entry], from: i32, dir: i32) -> i32 {
    let n = options.len() as i32;
    if n == 0 || dir == 0 {
        return from;
    }
    let mut at = if from < 0 {
        if dir > 0 { -1 } else { n }
    } else {
        from.min(n)
    };
    loop {
        at += dir;
        if at < 0 || at >= n {
            return from;
        }
        if !options[at as usize].disabled {
            return at;
        }
    }
}

/// The first enabled option whose label starts with `text` (case-insensitive), searching after `from` and wrapping. The
/// search is over one character at a time, so typing a letter repeatedly cycles through the options that start with it.
/// Returns -1 when no enabled option matches.
pub fn typeahead(options: &[Entry], from: i32, text: &str) -> i32 {
    let n = options.len() as i32;
    if n == 0 || text.is_empty() {
        return -1;
    }
    let needle = text.to_lowercase();
    let start = if from < 0 { -1 } else { from.min(n - 1) };
    for step in 1..=n {
        let at = (start + step).rem_euclid(n);
        let option = &options[at as usize];
        if !option.disabled && option.label.to_lowercase().starts_with(&needle) {
            return at;
        }
    }
    -1
}

/// How many group headings the menu shows: one for each option that starts a new non-empty group.
pub fn heading_count(options: &[Entry]) -> i32 {
    let mut count = 0;
    for (index, option) in options.iter().enumerate() {
        let group = &option.group;
        if !group.is_empty() && (index == 0 || options[index - 1].group != *group) {
            count += 1;
        }
    }
    count
}

/// Whether a label matches the Combobox filter: it contains the query, ignoring case. An empty query matches everything.
pub fn matches_query(label: &str, query: &str) -> bool {
    query.is_empty() || label.to_lowercase().contains(&query.to_lowercase())
}

/// The index of the option whose value is `value`, or -1 when none has it.
pub fn index_of(options: &[Entry], value: &str) -> i32 {
    options
        .iter()
        .position(|option| option.value == value)
        .map_or(-1, |index| index as i32)
}

/// Emits, in the calling module, the Slint side of the Select rules: `entries` (the Slint options as [`Entry`]s),
/// `filter_options` (the options whose label matches a Combobox query) and `wire_nav`, which answers the `SelectNav`
/// callbacks with the rules above. `$option` is the app's generated option struct (fields `value`, `label`, `disabled`,
/// `group`) and `$nav` is the app's generated `SelectNav` global. Expand it in the app crate, then call `wire_nav` once
/// per window that shows a Select. Without it the callbacks return their defaults and the menu cannot move.
#[macro_export]
macro_rules! wire_select {
    ($option:path, $nav:path $(,)?) => {
        /// The Slint options as the plain rules see them.
        pub fn entries(
            options: &::slint::ModelRc<$option>,
        ) -> ::std::vec::Vec<$crate::select::Entry> {
            use ::slint::Model as _;
            options
                .iter()
                .map(|option| $crate::select::Entry {
                    value: option.value.to_string(),
                    label: option.label.to_string(),
                    disabled: option.disabled,
                    group: option.group.to_string(),
                })
                .collect()
        }

        /// The options whose label matches `query` (see `matches_query`), in order.
        pub fn filter_options(
            options: &::slint::ModelRc<$option>,
            query: &str,
        ) -> ::slint::ModelRc<$option> {
            use ::slint::Model as _;
            let kept: ::std::vec::Vec<$option> = options
                .iter()
                .filter(|option| $crate::select::matches_query(option.label.as_str(), query))
                .collect();
            ::slint::ModelRc::new(::slint::VecModel::from(kept))
        }

        /// Wires the `SelectNav` search callbacks of one window to the rules.
        pub fn wire_nav(nav: &$nav) {
            nav.on_next_enabled(|options, from, dir| {
                $crate::select::next_enabled(&entries(&options), from, dir)
            });
            nav.on_typeahead(|options, from, text| {
                $crate::select::typeahead(&entries(&options), from, text.as_str())
            });
            nav.on_heading_count(|options| $crate::select::heading_count(&entries(&options)));
            nav.on_index_of(|options, value| {
                $crate::select::index_of(&entries(&options), value.as_str())
            });
            nav.on_filter_options(|options, query| filter_options(&options, query.as_str()));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(label: &str, disabled: bool) -> Entry {
        Entry {
            value: label.to_lowercase(),
            label: label.to_string(),
            disabled,
            group: String::new(),
        }
    }

    fn sample() -> Vec<Entry> {
        vec![
            entry("Apple", false),
            entry("Apricot", true),
            entry("Banana", false),
            entry("Blueberry", false),
            entry("Cherry", true),
        ]
    }

    #[test]
    fn down_skips_disabled_and_stops_at_the_end() {
        let o = sample();
        assert_eq!(next_enabled(&o, -1, 1), 0);
        assert_eq!(next_enabled(&o, 0, 1), 2);
        assert_eq!(
            next_enabled(&o, 3, 1),
            3,
            "no enabled option after the last one: stay"
        );
    }

    #[test]
    fn up_skips_disabled_and_stops_at_the_start() {
        let o = sample();
        assert_eq!(next_enabled(&o, 2, -1), 0);
        assert_eq!(next_enabled(&o, 0, -1), 0);
        assert_eq!(
            next_enabled(&o, -1, -1),
            3,
            "nothing highlighted: up starts from the end"
        );
        assert_eq!(
            next_enabled(&o, 5, -1),
            3,
            "from past the end (End key): the last enabled option, Cherry is disabled"
        );
    }

    #[test]
    fn all_disabled_or_empty_returns_from() {
        let o = vec![entry("A", true), entry("B", true)];
        assert_eq!(next_enabled(&o, -1, 1), -1);
        assert_eq!(next_enabled(&[], 0, 1), 0);
    }

    #[test]
    fn typeahead_jumps_to_the_next_enabled_match_and_cycles() {
        let o = sample();
        assert_eq!(typeahead(&o, -1, "b"), 2, "Banana first");
        assert_eq!(typeahead(&o, 2, "b"), 3, "then Blueberry");
        assert_eq!(typeahead(&o, 3, "b"), 2, "wraps back to Banana");
        assert_eq!(
            typeahead(&o, -1, "A"),
            0,
            "case-insensitive; disabled Apricot is skipped"
        );
        assert_eq!(typeahead(&o, -1, "c"), -1, "Cherry is disabled");
        assert_eq!(typeahead(&o, -1, "z"), -1);
        assert_eq!(typeahead(&o, -1, ""), -1);
    }

    #[test]
    fn heading_count_counts_each_new_group_once() {
        let mut o = sample();
        assert_eq!(heading_count(&o), 0);
        o[0].group = "Fruit".into();
        o[1].group = "Fruit".into();
        o[2].group = "Berries".into();
        o[4].group = "Berries".into();
        assert_eq!(
            heading_count(&o),
            3,
            "Fruit at 0, Berries at 2, and Berries again at 4 after an ungrouped row"
        );
    }

    #[test]
    fn filter_matches_case_insensitively_and_empty_keeps_all() {
        assert!(matches_query("Apricot", "APR"));
        assert!(
            matches_query("Blueberry", "berry"),
            "contains, not only prefix"
        );
        assert!(!matches_query("Banana", "kiwi"));
        assert!(matches_query("Banana", ""));
    }

    #[test]
    fn index_of_finds_the_value_or_minus_one() {
        let o = sample();
        assert_eq!(index_of(&o, "banana"), 2);
        assert_eq!(index_of(&o, "kiwi"), -1);
        assert_eq!(index_of(&o, ""), -1);
    }
}
