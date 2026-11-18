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
/// `count` is the number of items, and `columns` is the number of items in one row.
pub fn move_selection(
    selected: Option<usize>,
    count: usize,
    columns: usize,
    arrow: Arrow,
) -> Option<usize> {
    match (selected, arrow) {
        (None, _) => Some(0),
        (Some(row), Arrow::Up) if row >= columns => Some(row - columns),
        (Some(row), Arrow::Up) => Some(row),
        (Some(row), _) if row + columns < count => Some(row + columns),
        (Some(row), _) => Some(row),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 何も選ばれていないときに矢印キーを押すと、最初の項目が選ばれる。
    #[test]
    fn arrow_without_a_selection_selects_the_first_item() {
        assert_eq!(move_selection(None, 5, 1, Arrow::Down), Some(0));
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
}
