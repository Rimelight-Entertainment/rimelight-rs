//! Plain-Rust rules of the P0.17 batch 5 List and Table. The Slint components show the rows they are given. This module
//! decides the order of a sort, which rows a pick selects, how wide a resized column may be, and the sample rows the
//! gallery and the 10,000-row test show. It has no Slint types.
//!
//! The selection flag lives on each row, so a sort moves the selection with its row. A sort clears the shift anchor,
//! because the row order has changed.

use std::cmp::Ordering;

/// The narrowest a Table column may be, in logical pixels. A drag or a width below this is clamped to it.
pub const MIN_COLUMN_WIDTH: f32 = 48.0;

/// How many rows a pick can select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    /// At most one row is selected.
    Single,
    /// Any number of rows: Ctrl (or Space) toggles one, Shift selects the range from the anchor.
    Multi,
}

/// The sort of a Table: the sorted column, if any, and the direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SortState {
    pub column: Option<usize>,
    pub descending: bool,
}

impl SortState {
    /// The state after a click or Enter on a column header. A new column sorts ascending. The same column flips its
    /// direction, so a second click reverses the order.
    pub fn request(self, column: usize) -> SortState {
        if self.column == Some(column) {
            SortState {
                column: Some(column),
                descending: !self.descending,
            }
        } else {
            SortState {
                column: Some(column),
                descending: false,
            }
        }
    }
}

/// A cell's sort key: a finite number sorts before any text, numbers by value, text case-insensitively.
#[derive(Debug, PartialEq)]
struct SortKey {
    number: Option<f64>,
    text: String,
}

impl SortKey {
    fn of(cell: &str) -> SortKey {
        let trimmed = cell.trim();
        // A number wrapped in symbols ("★ 10", "$3.50", "75%") sorts as that number. Cells with letters around the
        // number ("Alien 3", "10 kg") stay text, so titles sort by their text.
        let bare = trimmed
            .trim_matches(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'));
        let wrapped_in_symbols = !bare.is_empty()
            && !trimmed
                .replacen(bare, "", 1)
                .chars()
                .any(char::is_alphanumeric);
        let number = trimmed.parse::<f64>().or_else(|e| {
            if wrapped_in_symbols {
                bare.parse::<f64>()
            } else {
                Err(e)
            }
        });
        match number {
            Ok(value) if value.is_finite() => SortKey {
                number: Some(value),
                text: String::new(),
            },
            _ => SortKey {
                number: None,
                text: trimmed.to_lowercase(),
            },
        }
    }

    /// A total order, so the sort never sees an inconsistent comparison (mixed numbers and text included).
    fn compare(&self, other: &SortKey) -> Ordering {
        match (self.number, other.number) {
            (Some(a), Some(b)) => a.total_cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self.text.cmp(&other.text),
        }
    }
}

/// The rows sorted by one cell each, given by `cell`. The sort is stable: rows with equal keys keep their order, also
/// when descending. The input is not changed.
pub fn sort_rows<T: Clone>(rows: &[T], cell: impl Fn(&T) -> String, descending: bool) -> Vec<T> {
    let mut keyed: Vec<(SortKey, usize)> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (SortKey::of(&cell(row)), index))
        .collect();
    keyed.sort_by(|a, b| {
        if descending {
            b.0.compare(&a.0)
        } else {
            a.0.compare(&b.0)
        }
    });
    keyed
        .into_iter()
        .map(|(_, index)| rows[index].clone())
        .collect()
}

/// Applies a pick to the selection flags of a list or table. `ctrl` toggles the row. `shift` selects every row from the
/// anchor to the row (replacing the selection), and keeps the anchor. A plain pick selects only the row and makes it the
/// anchor. In single mode a pick selects only the row; a toggle (`ctrl`) on a selected row clears it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub mode: SelectionMode,
    anchor: Option<usize>,
}

impl Selection {
    pub fn new(mode: SelectionMode) -> Selection {
        Selection { mode, anchor: None }
    }

    /// The row a shift-pick extends from, or `None` before any pick (and after a sort).
    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    /// Forgets the anchor. Call it after a sort, which changes the row order.
    pub fn reset_anchor(&mut self) {
        self.anchor = None;
    }

    pub fn pick(&mut self, flags: &mut [bool], index: usize, shift: bool, ctrl: bool) {
        if index >= flags.len() {
            return;
        }
        match self.mode {
            SelectionMode::Single => {
                if ctrl && flags[index] {
                    flags[index] = false;
                } else {
                    flags.fill(false);
                    flags[index] = true;
                }
                self.anchor = Some(index);
            }
            SelectionMode::Multi => {
                if let Some(anchor) = self.anchor.filter(|&a| a < flags.len()).filter(|_| shift) {
                    flags.fill(false);
                    let (from, to) = (anchor.min(index), anchor.max(index));
                    flags[from..=to].fill(true);
                    return;
                }
                if ctrl {
                    flags[index] = !flags[index];
                } else {
                    flags.fill(false);
                    flags[index] = true;
                }
                self.anchor = Some(index);
            }
        }
    }
}

/// The width of a column after a drag: its width at the start of the drag plus the drag distance, never below
/// [`MIN_COLUMN_WIDTH`].
pub fn clamp_width(width: f32) -> f32 {
    width.max(MIN_COLUMN_WIDTH)
}

/// The column names of the sample Table.
pub const SAMPLE_COLUMNS: [&str; 4] = ["Name", "Team", "Status", "Score"];

const TEAMS: [&str; 5] = ["Design", "Platform", "Support", "Growth", "Finance"];
const STATUSES: [&str; 3] = ["Open", "Review", "Done"];

/// `count` generated rows of the sample Table, one cell per column in [`SAMPLE_COLUMNS`]. The names are `Item 00001`
/// and up. The scores are `(index * 37) % 1000`, so they are numbers in a shuffled order. Deterministic.
pub fn sample_rows(count: usize) -> Vec<[String; 4]> {
    (0..count)
        .map(|index| {
            [
                format!("Item {:05}", index + 1),
                TEAMS[index % TEAMS.len()].to_string(),
                STATUSES[(index / 3) % STATUSES.len()].to_string(),
                ((index * 37) % 1000).to_string(),
            ]
        })
        .collect()
}

/// The labels of the sample List: `Entry 00001` and up.
pub fn sample_list_labels(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("Entry {:05}", index + 1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(rows: &[&str]) -> Vec<String> {
        rows.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_new_column_sorts_ascending_and_the_same_column_flips() {
        let first = SortState::default().request(2);
        assert_eq!(
            first,
            SortState {
                column: Some(2),
                descending: false
            }
        );
        let second = first.request(2);
        assert_eq!(
            second,
            SortState {
                column: Some(2),
                descending: true
            }
        );
        let third = second.request(2);
        assert!(!third.descending, "a third click sorts ascending again");
        let other = second.request(0);
        assert_eq!(
            other,
            SortState {
                column: Some(0),
                descending: false
            }
        );
    }

    #[test]
    fn numbers_sort_by_value_and_before_text() {
        let rows = names(&["10", "9", "x", "2", "", "NaN", "apple"]);
        let sorted = sort_rows(&rows, |r| r.clone(), false);
        assert_eq!(sorted, names(&["2", "9", "10", "", "apple", "NaN", "x"]));
    }

    #[test]
    fn numbers_wrapped_in_symbols_sort_by_value_but_titles_stay_text() {
        let rows = names(&["★ 10", "★ 9", "$3.50", "75%"]);
        let sorted = sort_rows(&rows, |r| r.clone(), false);
        assert_eq!(sorted, names(&["$3.50", "★ 9", "★ 10", "75%"]));
        let titles = names(&["Alien 3", "Alien", "Brave"]);
        let sorted = sort_rows(&titles, |r| r.clone(), false);
        assert_eq!(sorted, names(&["Alien", "Alien 3", "Brave"]));
    }

    #[test]
    fn text_sorts_case_insensitively_and_descending_reverses() {
        let rows = names(&["beta", "Alpha", "gamma"]);
        let asc = sort_rows(&rows, |r| r.clone(), false);
        assert_eq!(asc, names(&["Alpha", "beta", "gamma"]));
        let desc = sort_rows(&rows, |r| r.clone(), true);
        assert_eq!(desc, names(&["gamma", "beta", "Alpha"]));
    }

    #[test]
    fn equal_keys_keep_their_order_in_both_directions() {
        let rows = vec![("a", 1), ("b", 2), ("c", 1), ("d", 2)];
        let asc = sort_rows(&rows, |r| r.1.to_string(), false);
        assert_eq!(asc, vec![("a", 1), ("c", 1), ("b", 2), ("d", 2)]);
        let desc = sort_rows(&rows, |r| r.1.to_string(), true);
        assert_eq!(desc, vec![("b", 2), ("d", 2), ("a", 1), ("c", 1)]);
    }

    #[test]
    fn a_plain_pick_selects_only_that_row() {
        let mut sel = Selection::new(SelectionMode::Multi);
        let mut flags = vec![false; 5];
        sel.pick(&mut flags, 1, false, false);
        sel.pick(&mut flags, 3, false, false);
        assert_eq!(flags, vec![false, false, false, true, false]);
        assert_eq!(sel.anchor(), Some(3));
    }

    #[test]
    fn ctrl_toggles_and_keeps_the_others() {
        let mut sel = Selection::new(SelectionMode::Multi);
        let mut flags = vec![false; 4];
        sel.pick(&mut flags, 0, false, true);
        sel.pick(&mut flags, 2, false, true);
        assert_eq!(flags, vec![true, false, true, false]);
        sel.pick(&mut flags, 0, false, true);
        assert_eq!(flags, vec![false, false, true, false]);
    }

    #[test]
    fn shift_selects_the_range_from_the_anchor_and_keeps_it() {
        let mut sel = Selection::new(SelectionMode::Multi);
        let mut flags = vec![false; 6];
        sel.pick(&mut flags, 4, false, false);
        sel.pick(&mut flags, 1, true, false);
        assert_eq!(flags, vec![false, true, true, true, true, false]);
        assert_eq!(
            sel.anchor(),
            Some(4),
            "the anchor stays where the range began"
        );
        sel.pick(&mut flags, 5, true, false);
        assert_eq!(flags, vec![false, false, false, false, true, true]);
    }

    #[test]
    fn shift_without_an_anchor_is_a_plain_pick() {
        let mut sel = Selection::new(SelectionMode::Multi);
        let mut flags = vec![true, false, false];
        sel.pick(&mut flags, 2, true, false);
        assert_eq!(flags, vec![false, false, true]);
    }

    #[test]
    fn single_mode_keeps_one_row_and_a_toggle_clears_it() {
        let mut sel = Selection::new(SelectionMode::Single);
        let mut flags = vec![false; 3];
        sel.pick(&mut flags, 0, false, false);
        sel.pick(&mut flags, 2, true, true);
        assert_eq!(
            flags,
            vec![false, false, true],
            "shift and ctrl do not add rows in single mode"
        );
        sel.pick(&mut flags, 2, false, true);
        assert_eq!(flags, vec![false, false, false]);
    }

    #[test]
    fn picks_outside_the_list_do_nothing() {
        let mut sel = Selection::new(SelectionMode::Multi);
        let mut flags = vec![false; 2];
        sel.pick(&mut flags, 2, false, false);
        assert_eq!(flags, vec![false, false]);
        assert_eq!(sel.anchor(), None);
    }

    #[test]
    fn reset_anchor_forgets_the_range_start() {
        let mut sel = Selection::new(SelectionMode::Multi);
        let mut flags = vec![false; 3];
        sel.pick(&mut flags, 1, false, false);
        sel.reset_anchor();
        sel.pick(&mut flags, 2, true, false);
        assert_eq!(
            flags,
            vec![false, false, true],
            "no anchor, so shift is a plain pick"
        );
    }

    #[test]
    fn width_never_goes_below_the_minimum() {
        assert_eq!(clamp_width(200.0), 200.0);
        assert_eq!(clamp_width(10.0), MIN_COLUMN_WIDTH);
        assert_eq!(clamp_width(-40.0), MIN_COLUMN_WIDTH);
    }

    #[test]
    fn sample_rows_are_generated_and_deterministic() {
        let rows = sample_rows(10_000);
        assert_eq!(rows.len(), 10_000);
        assert_eq!(rows[0][0], "Item 00001");
        assert_eq!(rows[9_999][0], "Item 10000");
        assert_eq!(rows[1][3], "37");
        assert_eq!(rows, sample_rows(10_000));
        let labels = sample_list_labels(3);
        assert_eq!(labels, vec!["Entry 00001", "Entry 00002", "Entry 00003"]);
    }
}
