//! Plain-Rust state of the ST5.4 board template (`BoardPage`): columns (lanes) of card keys, a move of a card to another
//! column, the column a keyboard step reaches, and the column under a drag. It has no Slint types. The page holds card
//! keys and column indices; the card data itself stays with the app.

/// One column of the board: its name and its card keys in order, top first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lane {
    pub name: String,
    pub cards: Vec<String>,
}

/// Why a board change was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoardError {
    /// No card on the board has this key.
    UnknownCard(String),
    /// There is no column at this index.
    UnknownColumn(usize),
    /// A card with this key is already on the board, so it is not added again.
    DuplicateCard(String),
}

/// The columns of a board.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Board {
    lanes: Vec<Lane>,
}

impl Board {
    /// A board with these columns, in order.
    pub fn new(lanes: Vec<Lane>) -> Board {
        Board { lanes }
    }

    /// The columns in order.
    pub fn lanes(&self) -> &[Lane] {
        &self.lanes
    }

    /// How many columns the board has.
    pub fn column_count(&self) -> usize {
        self.lanes.len()
    }

    /// The column index of the card `key`, or `None` when no column has it.
    pub fn column_of(&self, key: &str) -> Option<usize> {
        self.lanes
            .iter()
            .position(|lane| lane.cards.iter().any(|card| card == key))
    }

    /// Moves the card `key` to the end of column `to`. Moving a card to the column it is in changes nothing and is not an
    /// error.
    pub fn move_card(&mut self, key: &str, to: usize) -> Result<(), BoardError> {
        if to >= self.lanes.len() {
            return Err(BoardError::UnknownColumn(to));
        }
        let from = self
            .column_of(key)
            .ok_or_else(|| BoardError::UnknownCard(key.to_string()))?;
        if from == to {
            return Ok(());
        }
        self.lanes[from].cards.retain(|card| card != key);
        self.lanes[to].cards.push(key.to_string());
        Ok(())
    }

    /// Adds the card `key` to the end of column `column`. A key that is already on the board is refused.
    pub fn add_card(&mut self, column: usize, key: &str) -> Result<(), BoardError> {
        if column >= self.lanes.len() {
            return Err(BoardError::UnknownColumn(column));
        }
        if self.column_of(key).is_some() {
            return Err(BoardError::DuplicateCard(key.to_string()));
        }
        self.lanes[column].cards.push(key.to_string());
        Ok(())
    }

    /// The column one step away from the card `key` (`step` is -1 for the left, 1 for the right). `None` when the card is
    /// unknown or the step would leave the board. This is what the keyboard move uses.
    pub fn neighbour(&self, key: &str, step: i32) -> Option<usize> {
        let from = self.column_of(key)? as i64;
        let target = from + i64::from(step);
        if target < 0 || target >= self.lanes.len() as i64 {
            None
        } else {
            Some(target as usize)
        }
    }
}

/// The column under a point in a row of columns, for a drag. `x` is in px from the left of the first column, `column_width`
/// is each column's width, `gap` the space between columns, and `count` the number of columns. Returns the column index, or
/// -1 when `x` is left of the board, past its last column, or in a gap. Slint passes this rule the same values, so the
/// page calls it through its `column-at` callback.
pub fn drop_column(x: f32, column_width: f32, gap: f32, count: usize) -> i32 {
    if count == 0 || column_width <= 0.0 || !x.is_finite() || x < 0.0 {
        return -1;
    }
    let pitch = column_width + gap.max(0.0);
    let index = (x / pitch).floor() as usize;
    if index >= count {
        return -1;
    }
    let within = x - index as f32 * pitch;
    if within > column_width {
        return -1;
    }
    index as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> Board {
        Board::new(vec![
            Lane {
                name: "To do".into(),
                cards: vec!["A-1".into(), "A-2".into()],
            },
            Lane {
                name: "Doing".into(),
                cards: vec!["A-3".into()],
            },
            Lane {
                name: "Done".into(),
                cards: vec![],
            },
        ])
    }

    #[test]
    fn a_card_moves_to_the_end_of_another_column() {
        let mut b = board();
        assert_eq!(b.move_card("A-1", 2), Ok(()));
        assert_eq!(b.column_of("A-1"), Some(2));
        assert_eq!(b.lanes()[0].cards, vec!["A-2".to_string()]);
        assert_eq!(b.lanes()[2].cards, vec!["A-1".to_string()]);
    }

    #[test]
    fn a_move_to_the_same_column_changes_nothing() {
        let mut b = board();
        let before = b.clone();
        assert_eq!(b.move_card("A-2", 0), Ok(()));
        assert_eq!(b, before);
    }

    #[test]
    fn a_move_names_what_it_refused() {
        let mut b = board();
        assert_eq!(
            b.move_card("nope", 1),
            Err(BoardError::UnknownCard("nope".into()))
        );
        assert_eq!(b.move_card("A-1", 7), Err(BoardError::UnknownColumn(7)));
    }

    #[test]
    fn adding_a_card_refuses_a_duplicate_and_a_bad_column() {
        let mut b = board();
        assert_eq!(b.add_card(2, "A-9"), Ok(()));
        assert_eq!(b.column_of("A-9"), Some(2));
        assert_eq!(
            b.add_card(1, "A-9"),
            Err(BoardError::DuplicateCard("A-9".into()))
        );
        assert_eq!(b.add_card(3, "A-10"), Err(BoardError::UnknownColumn(3)));
    }

    #[test]
    fn a_keyboard_step_stays_on_the_board() {
        let b = board();
        assert_eq!(b.neighbour("A-3", 1), Some(2));
        assert_eq!(b.neighbour("A-3", -1), Some(0));
        assert_eq!(b.neighbour("A-1", -1), None, "already in the first column");
        assert_eq!(b.neighbour("missing", 1), None);
    }

    #[test]
    fn the_drop_column_is_found_from_the_pointer_x() {
        // Columns 100 px wide with a 10 px gap: column 0 is 0..100, column 1 is 110..210, column 2 is 220..320.
        assert_eq!(drop_column(0.0, 100.0, 10.0, 3), 0);
        assert_eq!(
            drop_column(100.0, 100.0, 10.0, 3),
            0,
            "right edge is inside"
        );
        assert_eq!(drop_column(105.0, 100.0, 10.0, 3), -1, "in the gap");
        assert_eq!(drop_column(110.0, 100.0, 10.0, 3), 1);
        assert_eq!(drop_column(215.0, 100.0, 10.0, 3), -1, "gap after column 1");
        assert_eq!(drop_column(320.0, 100.0, 10.0, 3), 2);
        assert_eq!(
            drop_column(330.0, 100.0, 10.0, 3),
            -1,
            "past the last column"
        );
    }

    #[test]
    fn the_drop_column_refuses_nonsense_inputs() {
        assert_eq!(drop_column(-1.0, 100.0, 10.0, 3), -1);
        assert_eq!(drop_column(5.0, 0.0, 10.0, 3), -1);
        assert_eq!(drop_column(5.0, 100.0, 10.0, 0), -1);
        assert_eq!(drop_column(f32::NAN, 100.0, 10.0, 3), -1);
    }
}
