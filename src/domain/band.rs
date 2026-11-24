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
}
