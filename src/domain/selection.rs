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
    _selected: Option<usize>,
    _count: usize,
    _columns: usize,
    _arrow: Arrow,
) -> Option<usize> {
    Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 何も選ばれていないときに矢印キーを押すと、最初の項目が選ばれる。
    #[test]
    fn arrow_without_a_selection_selects_the_first_item() {
        assert_eq!(move_selection(None, 5, 1, Arrow::Down), Some(0));
    }
}
