//! Plain-Rust state of the ST5.4 list template (`ListPage`): the filter text, the sort, and the selection of its rows. The
//! rows are cells of text, one `Vec<String>` per row in column order, so the rules have no Slint types.
//!
//! Display order: a row is shown when the filter matches one of its cells, and the shown rows are sorted by the sorted
//! column (the stable sort of `table::sort_rows`). A pick from the page names a position in that display order, and the
//! selection is kept per row, so a sort or a filter moves the selection with its row. A filter hides rows but keeps their
//! selection. A sort or a new filter forgets the shift anchor, the same as the Table.

use crate::table::{Selection, SelectionMode, SortState, sort_rows};

/// True when the filter text is empty or some cell contains it, ignoring case and surrounding spaces.
pub fn matches_filter(cells: &[String], filter: &str) -> bool {
    let needle = filter.trim().to_lowercase();
    needle.is_empty()
        || cells
            .iter()
            .any(|cell| cell.to_lowercase().contains(&needle))
}

/// The state of one list: its filter, its sort, and which rows are selected.
#[derive(Clone, Debug)]
pub struct ListModel {
    filter: String,
    sort: SortState,
    selection: Selection,
    /// One flag per row, by the row's index in the data (not its display position).
    selected: Vec<bool>,
}

impl ListModel {
    /// An empty list with no filter and no sort. `mode` says whether one row or many can be selected.
    pub fn new(mode: SelectionMode) -> ListModel {
        ListModel {
            filter: String::new(),
            sort: SortState::default(),
            selection: Selection::new(mode),
            selected: Vec::new(),
        }
    }

    /// The filter text as the user typed it.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// The sort in force.
    pub fn sort(&self) -> SortState {
        self.sort
    }

    /// Sets the filter text. The shift anchor is forgotten, because the display order may change.
    pub fn set_filter(&mut self, text: &str) {
        self.filter = text.to_string();
        self.selection.reset_anchor();
    }

    /// A click or Enter on a column header. The same column flips its direction, a new one sorts ascending.
    pub fn request_sort(&mut self, column: usize) {
        self.sort = self.sort.request(column);
        self.selection.reset_anchor();
    }

    /// The data row indices in display order: the rows the filter keeps, sorted by the sorted column (if any).
    /// `rows` is the data; a row with fewer cells than the sorted column sorts as empty text.
    pub fn order(&self, rows: &[Vec<String>]) -> Vec<usize> {
        let visible: Vec<usize> = (0..rows.len())
            .filter(|&index| matches_filter(&rows[index], &self.filter))
            .collect();
        match self.sort.column {
            Some(column) => sort_rows(
                &visible,
                |&index| rows[index].get(column).cloned().unwrap_or_default(),
                self.sort.descending,
            ),
            None => visible,
        }
    }

    /// A pick on the row at `display_index` (its position in [`ListModel::order`]). `ctrl` toggles that row, `shift` selects
    /// the range from the anchor in display order, and a plain pick selects only that row. Only the rows on screen are
    /// changed; rows the filter hides keep their selection.
    pub fn pick(&mut self, rows: &[Vec<String>], display_index: usize, shift: bool, ctrl: bool) {
        self.selected.resize(rows.len(), false);
        let order = self.order(rows);
        let mut shown: Vec<bool> = order.iter().map(|&row| self.selected[row]).collect();
        self.selection.pick(&mut shown, display_index, shift, ctrl);
        for (flag, &row) in shown.iter().zip(&order) {
            self.selected[row] = *flag;
        }
    }

    /// Whether the data row at `row` is selected.
    pub fn is_selected(&self, row: usize) -> bool {
        self.selected.get(row).copied().unwrap_or(false)
    }

    /// The selected data row indices, in data order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selected
            .iter()
            .enumerate()
            .filter(|(_, flag)| **flag)
            .map(|(index, _)| index)
            .collect()
    }

    /// Clears every selection, for example after a bulk action has run.
    pub fn clear_selection(&mut self) {
        self.selected.fill(false);
        self.selection.reset_anchor();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Vec<String>> {
        [
            ["Ada", "Design", "12"],
            ["Brook", "Platform", "7"],
            ["Chen", "Design", "30"],
            ["Dara", "Support", "7"],
        ]
        .iter()
        .map(|row| row.iter().map(|cell| cell.to_string()).collect())
        .collect()
    }

    #[test]
    fn filter_matches_any_cell_ignoring_case_and_spaces() {
        let cells = vec!["Ada".to_string(), "Design".to_string()];
        assert!(matches_filter(&cells, ""));
        assert!(matches_filter(&cells, "  des "));
        assert!(matches_filter(&cells, "ADA"));
        assert!(!matches_filter(&cells, "platform"));
    }

    #[test]
    fn the_display_order_keeps_data_order_until_a_sort_or_filter() {
        let mut list = ListModel::new(SelectionMode::Multi);
        assert_eq!(list.order(&rows()), vec![0, 1, 2, 3]);
        list.set_filter("design");
        assert_eq!(list.order(&rows()), vec![0, 2]);
    }

    #[test]
    fn a_sort_flips_on_the_same_column_and_is_stable() {
        let mut list = ListModel::new(SelectionMode::Multi);
        list.request_sort(2);
        // Scores 12, 7, 30, 7: the two 7s keep their data order (Brook, then Dara).
        assert_eq!(list.order(&rows()), vec![1, 3, 0, 2]);
        list.request_sort(2);
        assert_eq!(list.order(&rows()), vec![2, 0, 1, 3]);
        assert!(list.sort().descending);
    }

    #[test]
    fn a_plain_pick_selects_one_row_and_a_toggle_adds_another() {
        let data = rows();
        let mut list = ListModel::new(SelectionMode::Multi);
        list.pick(&data, 1, false, false);
        assert_eq!(list.selected_rows(), vec![1]);
        list.pick(&data, 3, false, true);
        assert_eq!(list.selected_rows(), vec![1, 3]);
        list.pick(&data, 1, false, true);
        assert_eq!(list.selected_rows(), vec![3]);
    }

    #[test]
    fn a_shift_pick_selects_the_range_in_display_order() {
        let data = rows();
        let mut list = ListModel::new(SelectionMode::Multi);
        list.request_sort(2);
        // Display order is [1, 3, 0, 2]. Anchor on display position 0 (Brook), shift to position 2 (Ada).
        list.pick(&data, 0, false, false);
        list.pick(&data, 2, true, false);
        assert_eq!(list.selected_rows(), vec![0, 1, 3]);
    }

    #[test]
    fn a_filter_hides_rows_but_keeps_their_selection() {
        let data = rows();
        let mut list = ListModel::new(SelectionMode::Multi);
        list.pick(&data, 1, false, false);
        list.set_filter("design");
        list.pick(&data, 0, false, false);
        assert_eq!(
            list.selected_rows(),
            vec![0, 1],
            "row 1 is hidden by the filter, so its selection is kept"
        );
        list.set_filter("");
        assert!(list.is_selected(1));
    }

    #[test]
    fn single_mode_keeps_one_row_and_clear_removes_all() {
        let data = rows();
        let mut list = ListModel::new(SelectionMode::Single);
        list.pick(&data, 0, false, false);
        list.pick(&data, 2, false, true);
        assert_eq!(list.selected_rows(), vec![2]);
        list.clear_selection();
        assert!(list.selected_rows().is_empty());
    }

    #[test]
    fn a_pick_past_the_end_does_nothing() {
        let data = rows();
        let mut list = ListModel::new(SelectionMode::Multi);
        list.pick(&data, 9, false, false);
        assert!(list.selected_rows().is_empty());
    }
}
