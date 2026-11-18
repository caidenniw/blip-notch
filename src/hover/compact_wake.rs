use std::time::{Duration, Instant};

const DWELL: Duration = Duration::from_millis(600);
const MOTION_TOLERANCE: f64 = 4.0;

#[derive(Default)]
pub(super) enum CompactWake {
    #[default]
    Idle,
    Waiting { x: f64, y: f64, since: Instant },
    BlockedUntilExit,
}

impl CompactWake {
    // Coordinates are logical pixels relative to the center of the notch's top edge.
    pub(super) fn update(&mut self, x: f64, y: f64, pressed: bool, now: Instant) -> bool {
        if !(-30.0..30.0).contains(&x) || !(0.0..2.0).contains(&y) {
            *self = Self::Idle;
        } else if pressed {
            *self = Self::BlockedUntilExit;
        } else {
            match *self {
                Self::BlockedUntilExit => {}
                Self::Waiting { x: start_x, y: start_y, since }
                    if (x - start_x).hypot(y - start_y) <= MOTION_TOLERANCE =>
                {
                    return now.duration_since(since) >= DWELL;
                }
                _ => *self = Self::Waiting { x, y, since: now },
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_restarts_dwell_but_jitter_is_allowed() {
        let mut wake = CompactWake::Idle;
        let start = Instant::now();
        let t = |ms| start + Duration::from_millis(ms);
        let mut sample = |x, ms| wake.update(x, 1.0, false, t(ms));
        assert!(!sample(-8.0, 0));
        assert!(!sample(-5.0, 300));
        assert!(!sample(-2.0, 540)); // Six pixels from the original anchor.
        assert!(!sample(0.0, 1139));
        assert!(sample(1.0, 1140));
    }

    #[test]
    fn click_or_drag_blocks_until_pointer_leaves_and_dwells_again() {
        let start = Instant::now();
        for initial_x in [0.0, -50.0] {
            let mut wake = CompactWake::Idle;
            let mut sample = |x, pressed, ms| {
                wake.update(x, 0.0, pressed, start + Duration::from_millis(ms))
            };
            assert!(!sample(initial_x, initial_x < -30.0, 0));
            assert!(!sample(0.0, true, 599));
            assert!(!sample(0.0, true, 900));
            assert!(!sample(0.0, false, 1800));
            assert!(!sample(30.0, false, 2000));
            assert!(!sample(0.0, false, 2100));
            assert!(sample(0.0, false, 2700));
        }
    }

    #[test]
    fn leaving_the_strip_discards_partial_dwell() {
        let start = Instant::now();
        for (x, y) in [(-30.1, 0.0), (30.0, 0.0), (0.0, -0.1), (0.0, 2.0)] {
            let mut wake = CompactWake::Idle;
            assert!(!wake.update(-30.0, 0.0, false, start));
            assert!(!wake.update(x, y, false, start + DWELL));
            assert!(!wake.update(-30.0, 0.0, false, start + DWELL * 2));
            assert!(wake.update(-30.0, 0.0, false, start + DWELL * 3));
        }
    }
}
