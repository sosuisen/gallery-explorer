use std::time::{Duration, Instant};

/// The longest time between the two presses of a double click.
const INTERVAL: Duration = Duration::from_millis(500);

/// Finds double clicks in a stream of presses on list rows.
///
/// The list control reports no double click, so the app counts presses itself.
#[derive(Debug, Default)]
pub struct DoubleClick {
    last: Option<(usize, Instant)>,
}

impl DoubleClick {
    /// Records a press on a row. Returns `true` when the press completes a double click.
    pub fn press(&mut self, row: usize, at: Instant) -> bool {
        let is_double = self.last.is_some_and(|(last_row, last_at)| {
            last_row == row && at.saturating_duration_since(last_at) <= INTERVAL
        });
        // A third quick press must start a new double click, so the completed one is forgotten.
        self.last = if is_double { None } else { Some((row, at)) };
        is_double
    }

    /// Forgets the last press. The rows change when the folder changes.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 同じ行を短い間隔で2回押すと、ダブルクリックになる。
    #[test]
    fn two_quick_presses_on_the_same_row_are_a_double_click() {
        let mut clicks = DoubleClick::default();
        let start = Instant::now();

        assert!(!clicks.press(1, start));
        assert!(clicks.press(1, start + INTERVAL));
    }

    // 間隔が長すぎる2回の押下は、ダブルクリックにならない。
    #[test]
    fn two_slow_presses_are_not_a_double_click() {
        let mut clicks = DoubleClick::default();
        let start = Instant::now();
        clicks.press(1, start);

        assert!(!clicks.press(1, start + INTERVAL + Duration::from_millis(1)));
    }

    // 別の行への2回の押下は、ダブルクリックにならない。
    #[test]
    fn presses_on_different_rows_are_not_a_double_click() {
        let mut clicks = DoubleClick::default();
        let start = Instant::now();
        clicks.press(1, start);

        assert!(!clicks.press(2, start));
    }

    // ダブルクリックの直後の3回目の押下は、新しいダブルクリックの1回目になる。
    #[test]
    fn third_quick_press_starts_a_new_double_click() {
        let mut clicks = DoubleClick::default();
        let start = Instant::now();
        clicks.press(1, start);
        clicks.press(1, start);

        assert!(!clicks.press(1, start));
        assert!(clicks.press(1, start));
    }

    // リセットのあとの押下は、1回目として数えられる。
    #[test]
    fn press_after_a_reset_counts_as_the_first() {
        let mut clicks = DoubleClick::default();
        let start = Instant::now();
        clicks.press(1, start);

        clicks.reset();

        assert!(!clicks.press(1, start));
    }
}
