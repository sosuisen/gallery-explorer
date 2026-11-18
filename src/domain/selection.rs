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
    _columns: usize,
    _arrow: Arrow,
) -> Option<usize> {
    match selected {
        None => Some(0),
        Some(row) if row + 1 < count => Some(row + 1),
        Some(row) => Some(row),
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
}
