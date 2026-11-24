//! The rectangle that the user drags over the items to select them.

/// A point in the area of the items, in device independent pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A rectangle whose left is not right of its right, and whose top is not below its bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    /// The rectangle with `a` and `b` at opposite corners.
    pub fn between(a: Point, b: Point) -> Self {
        Self {
            left: a.x.min(b.x),
            top: a.y.min(b.y),
            right: a.x.max(b.x),
            bottom: a.y.max(b.y),
        }
    }

    /// Whether the two rectangles share an area. Touching edges share none.
    fn overlaps(&self, other: Rect) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}

/// Where the items lie in their area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemLayout {
    margin: f64,
    columns: usize,
    item_width: f64,
    item_height: f64,
}

impl ItemLayout {
    /// Rows of `row_height` below each other, as wide as the area, inside `margin`.
    pub fn list(margin: f64, row_height: f64) -> Self {
        Self {
            margin,
            columns: 1,
            // Not INFINITY: the left edge multiplies the width by zero, which would give NaN.
            item_width: f64::MAX,
            item_height: row_height,
        }
    }

    /// Tiles of `width` by `height`, `columns` in a row, filling the rows from the left, inside
    /// `margin`.
    ///
    /// # Panics
    ///
    /// Panics if `columns` is zero.
    pub fn grid(margin: f64, columns: usize, width: f64, height: f64) -> Self {
        assert!(columns > 0, "a row holds at least one item");
        Self {
            margin,
            columns,
            item_width: width,
            item_height: height,
        }
    }

    /// The items among the first `count` items that overlap `rect`, in ascending order.
    pub fn items_in(&self, rect: Rect, count: usize) -> Vec<usize> {
        (0..count)
            .filter(|&item| self.bounds(item).overlaps(rect))
            .collect()
    }

    fn bounds(&self, item: usize) -> Rect {
        let left = self.margin + self.item_width * (item % self.columns) as f64;
        let top = self.margin + self.item_height * (item / self.columns) as f64;
        Rect {
            left,
            top,
            right: left + self.item_width,
            bottom: top + self.item_height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2点から作る矩形は、どちらの向きにドラッグしても同じである。
    #[test]
    fn rectangle_is_the_same_in_any_drag_direction() {
        let down_right = Rect::between(Point::new(10.0, 20.0), Point::new(30.0, 50.0));
        let up_left = Rect::between(Point::new(30.0, 50.0), Point::new(10.0, 20.0));

        assert_eq!(down_right, up_left);
        assert_eq!(
            down_right,
            Rect {
                left: 10.0,
                top: 20.0,
                right: 30.0,
                bottom: 50.0,
            }
        );
    }

    // 一覧では、矩形と高さが重なる行が選ばれる。
    #[test]
    fn list_rows_that_overlap_the_rectangle_in_height_are_in_it() {
        // Rows of 10px below a margin of 4px: row 1 spans 14 to 24, row 2 spans 24 to 34.
        let layout = ItemLayout::list(4.0, 10.0);
        let rect = Rect::between(Point::new(500.0, 20.0), Point::new(600.0, 25.0));

        assert_eq!(layout.items_in(rect, 5), [1, 2]);
    }

    // グリッドでは、矩形と重なるタイルが選ばれる。
    #[test]
    fn grid_tiles_that_overlap_the_rectangle_are_in_it() {
        // Three tiles of 10px by 20px in a row, below and right of a margin of 4px:
        //   0: 4..14   1: 14..24   2: 24..34   (x)   row 0: 4..24 (y)
        //   3          4           5                 row 1: 24..44
        let layout = ItemLayout::grid(4.0, 3, 10.0, 20.0);
        let rect = Rect::between(Point::new(20.0, 30.0), Point::new(26.0, 50.0));

        assert_eq!(layout.items_in(rect, 7), [4, 5]);
    }

    // 項目の外の矩形では、何も選ばれない。端が接するだけの項目も選ばれない。
    #[test]
    fn rectangle_outside_the_items_holds_none() {
        let layout = ItemLayout::grid(4.0, 3, 10.0, 20.0);
        let in_margin = Rect::between(Point::new(0.0, 0.0), Point::new(4.0, 50.0));
        let below = Rect::between(Point::new(0.0, 44.0), Point::new(50.0, 60.0));

        assert_eq!(layout.items_in(in_margin, 6), Vec::<usize>::new());
        assert_eq!(layout.items_in(below, 6), Vec::<usize>::new());
    }
}
