//! The rectangle that the user drags over the items to select them.

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
}
