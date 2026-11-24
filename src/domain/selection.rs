use std::collections::BTreeSet;

/// The selected items, by row.
///
/// The focus is the row that the cursor keys move from, and the row that Enter opens.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Selection {
    rows: BTreeSet<usize>,
    focus: Option<usize>,
}

impl Selection {
    /// Selects `rows`, with the focus on `focus`.
    pub fn new(rows: impl IntoIterator<Item = usize>, focus: Option<usize>) -> Self {
        Self {
            rows: rows.into_iter().collect(),
            focus,
        }
    }

    /// The selected rows, in ascending order.
    pub fn rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.rows.iter().copied()
    }

    pub fn focus(&self) -> Option<usize> {
        self.focus
    }

    /// Selects `row` and nothing else.
    pub fn select_only(&mut self, row: usize) {
        self.rows = BTreeSet::from([row]);
        self.focus = Some(row);
    }

    /// Adds `row` to the selection and moves the focus to it.
    pub fn add(&mut self, row: usize) {
        self.rows.insert(row);
        self.focus = Some(row);
    }

    /// Takes `row` out of the selection. The focus stays where it is.
    pub fn remove(&mut self, row: usize) {
        self.rows.remove(&row);
    }

    pub fn contains(&self, row: usize) -> bool {
        self.rows.contains(&row)
    }
}

/// A cursor key that moves the selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arrow {
    Up,
    Down,
    Left,
    Right,
}

/// Returns the item selected after `arrow` is pressed.
///
/// `count` is the number of items, and `columns` is the number of items in one row. The items
/// fill the rows from the left, so the last row may be shorter.
///
/// # Panics
///
/// Panics if `columns` is zero.
pub fn move_selection(
    selected: Option<usize>,
    count: usize,
    columns: usize,
    arrow: Arrow,
) -> Option<usize> {
    assert!(columns > 0, "a row holds at least one item");
    let last = count.checked_sub(1)?;
    let Some(item) = selected else {
        return Some(0);
    };
    let moved = match arrow {
        Arrow::Up => item.checked_sub(columns).unwrap_or(item),
        Arrow::Left => item.saturating_sub(1),
        Arrow::Right => (item + 1).min(last),
        Arrow::Down if item / columns < last / columns => (item + columns).min(last),
        Arrow::Down => item,
    };
    Some(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(selection: &Selection) -> Vec<usize> {
        selection.rows().collect()
    }

    // 1つだけ選ぶと、前の選択は消える。
    #[test]
    fn selecting_only_one_item_drops_the_earlier_selection() {
        let mut selection = Selection::default();
        selection.select_only(1);

        selection.select_only(3);

        assert_eq!(rows(&selection), [3]);
        assert_eq!(selection.focus(), Some(3));
    }

    // 加えると、前の選択に足される。
    #[test]
    fn adding_an_item_keeps_the_earlier_selection() {
        let mut selection = Selection::default();
        selection.select_only(3);

        selection.add(1);

        assert_eq!(rows(&selection), [1, 3]);
        assert_eq!(selection.focus(), Some(1));
    }

    // 外すと、その項目だけが選択から消える。
    #[test]
    fn removing_an_item_keeps_the_others() {
        let mut selection = Selection::default();
        selection.select_only(1);
        selection.add(3);

        selection.remove(1);

        assert_eq!(rows(&selection), [3]);
        assert!(!selection.contains(1));
        assert!(selection.contains(3));
    }

    // 何も選ばれていないときに矢印キーを押すと、最初の項目が選ばれる。
    #[test]
    fn arrow_without_a_selection_selects_the_first_item() {
        assert_eq!(move_selection(None, 5, 1, Arrow::Down), Some(0));
    }

    // 項目がないときは、矢印キーを押しても何も選ばれない。
    #[test]
    fn arrow_selects_nothing_without_items() {
        assert_eq!(move_selection(None, 0, 1, Arrow::Down), None);
    }

    // 下キーを押すと、1つ下の行が選ばれる。
    #[test]
    fn down_selects_the_next_row() {
        assert_eq!(move_selection(Some(1), 5, 1, Arrow::Down), Some(2));
    }

    // 最後の行で下キーを押しても、選択は動かない。
    #[test]
    fn down_on_the_last_row_keeps_the_selection() {
        assert_eq!(move_selection(Some(4), 5, 1, Arrow::Down), Some(4));
    }

    // 上キーを押すと、1つ上の行が選ばれる。最初の行では、選択は動かない。
    #[test]
    fn up_selects_the_previous_row_and_stops_at_the_first() {
        assert_eq!(move_selection(Some(2), 5, 1, Arrow::Up), Some(1));
        assert_eq!(move_selection(Some(0), 5, 1, Arrow::Up), Some(0));
    }

    // 1行に複数の項目が並ぶときは、上下キーで、真上と真下の項目が選ばれる。
    #[test]
    fn up_and_down_move_by_one_row_of_several_items() {
        assert_eq!(move_selection(Some(1), 10, 4, Arrow::Down), Some(5));
        assert_eq!(move_selection(Some(5), 10, 4, Arrow::Up), Some(1));
        assert_eq!(move_selection(Some(2), 10, 4, Arrow::Up), Some(2));
    }

    // 下の行が短く、真下に項目がないときは、下キーで最後の項目が選ばれる。
    #[test]
    fn down_selects_the_last_item_when_nothing_is_right_below() {
        assert_eq!(move_selection(Some(7), 10, 4, Arrow::Down), Some(9));
        assert_eq!(move_selection(Some(9), 10, 4, Arrow::Down), Some(9));
    }

    // グリッドの最後の行では、下キーを押しても、選択は動かない。
    #[test]
    fn down_on_the_last_row_of_tiles_keeps_the_selection() {
        assert_eq!(move_selection(Some(8), 10, 4, Arrow::Down), Some(8));
    }

    // 左右キーを押すと、1つ前と1つ後の項目が選ばれる。行の端では、隣の行へ移る。
    #[test]
    fn left_and_right_select_the_previous_and_next_items() {
        assert_eq!(move_selection(Some(4), 10, 4, Arrow::Left), Some(3));
        assert_eq!(move_selection(Some(3), 10, 4, Arrow::Right), Some(4));
    }

    // 最初の項目で左キー、最後の項目で右キーを押しても、選択は動かない。
    #[test]
    fn left_and_right_stop_at_the_ends() {
        assert_eq!(move_selection(Some(0), 10, 4, Arrow::Left), Some(0));
        assert_eq!(move_selection(Some(9), 10, 4, Arrow::Right), Some(9));
    }
}
