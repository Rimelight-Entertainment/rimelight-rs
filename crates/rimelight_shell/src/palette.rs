//! Command palette rules (S9.3; PLAN 6.1), shared by every Rimelight app. Plain Rust, always compiled, so its tests run
//! without a window.
//!
//! An app builds its palette rows as [`Entry`] values from its own route list: each entry is a page title, its route with
//! any parameter filled in, and its search aliases. [`filter`] picks the entries a query matches, and [`step`] moves the
//! highlight on Up and Down. The markup (`@rimelight_shell/components/command-palette.slint`) draws the rows and reports
//! keys; each app answers those callbacks from these rules.

/// One palette row: a page title, its route with any parameter filled in, and its search aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: &'static str,
    pub route: String,
    pub aliases: &'static [&'static str],
}

/// Indices into `entries` whose title, route or one of whose aliases contains `query`, ignoring case. A blank query
/// (after trimming) matches every entry. The order is the entry order.
pub fn filter(query: &str, entries: &[Entry]) -> Vec<usize> {
    let needle = query.trim().to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            needle.is_empty()
                || entry.title.to_lowercase().contains(&needle)
                || entry.route.to_lowercase().contains(&needle)
                || entry
                    .aliases
                    .iter()
                    .any(|alias| alias.to_lowercase().contains(&needle))
        })
        .map(|(index, _)| index)
        .collect()
}

/// The highlight after an Up (`delta` -1) or Down (`delta` +1) press. It stops at the first and last row rather than
/// wrapping. With no rows the highlight is -1 (nothing).
pub fn step(highlight: i32, rows: usize, delta: i32) -> i32 {
    if rows == 0 {
        return -1;
    }
    (highlight + delta).clamp(0, rows as i32 - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &'static str, route: &str, aliases: &'static [&'static str]) -> Entry {
        Entry {
            title,
            route: route.to_owned(),
            aliases,
        }
    }

    fn sample() -> Vec<Entry> {
        vec![
            entry("Home", "/", &[]),
            entry("Pipelines", "/p/alpha/pipelines", &["Builds", "Jobs"]),
            entry("Issues", "/p/alpha/issues", &[]),
        ]
    }

    #[test]
    fn a_blank_query_matches_every_entry() {
        let all = sample();
        assert_eq!(filter("", &all), [0, 1, 2]);
        assert_eq!(filter("   ", &all), [0, 1, 2]);
    }

    #[test]
    fn title_route_and_alias_match_ignoring_case_and_surrounding_space() {
        let all = sample();
        assert_eq!(filter("HOME", &all), [0], "a title");
        assert_eq!(filter("ALPHA/ISSUES", &all), [2], "a route");
        assert_eq!(filter("builds", &all), [1], "an alias");
        assert_eq!(filter("  jobs ", &all), [1], "the query is trimmed");
    }

    #[test]
    fn no_match_gives_no_entries() {
        assert!(filter("zz-no-such-page", &sample()).is_empty());
        assert!(filter("anything", &[]).is_empty());
    }

    #[test]
    fn step_moves_one_row_and_stops_at_both_ends() {
        assert_eq!(step(-1, 3, 1), 0, "the first Down picks the first row");
        assert_eq!(step(1, 3, 1), 2, "Down moves one row");
        assert_eq!(step(2, 3, 1), 2, "Down stops at the last row");
        assert_eq!(step(1, 3, -1), 0, "Up moves one row");
        assert_eq!(step(0, 3, -1), 0, "Up stops at the first row");
    }

    #[test]
    fn step_with_no_rows_is_nothing() {
        assert_eq!(step(0, 0, 1), -1, "no rows, no highlight");
        assert_eq!(step(-1, 0, -1), -1, "no rows, no highlight");
    }
}
