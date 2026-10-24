//! Easing curves and tweens matching the CSS transitions of the old UI.
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    /// cubic-bezier(.25,.1,.25,1)
    Ease,
    /// cubic-bezier(.42,0,.58,1)
    EaseInOut,
    /// `--ease-out`: cubic-bezier(.22,1,.36,1)
    EaseOut,
    /// `--ease-close`: cubic-bezier(.32,.72,0,1)
    EaseClose,
    /// water bar: cubic-bezier(.2,.8,.2,1)
    WaterBar,
    /// `--spring`: CSS `linear()` table, overshoots ~1.1% near 77-80%.
    Spring,
}

/// `--spring` stops at t = i/30 (i = 0..=30).
const SPRING: [f32; 31] = [
    0.0, 0.024, 0.0842, 0.1666, 0.2603, 0.3575, 0.4528, 0.5424, 0.6241, 0.6968, 0.76, 0.8139,
    0.8591, 0.8962, 0.9262, 0.95, 0.9685, 0.9825, 0.9929, 1.0002, 1.0053, 1.0085, 1.0102, 1.011,
    1.011, 1.0106, 1.0098, 1.0088, 1.0077, 1.0066, 1.0,
];

pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bez = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    };
    let dbez = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    };
    // Newton first, bisection fallback.
    let mut t = x;
    for _ in 0..8 {
        let err = bez(x1, x2, t) - x;
        if err.abs() < 1e-6 {
            return bez(y1, y2, t);
        }
        let d = dbez(x1, x2, t);
        if d.abs() < 1e-6 {
            break;
        }
        t -= err / d;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    t = x;
    for _ in 0..40 {
        let v = bez(x1, x2, t);
        if (v - x).abs() < 1e-6 {
            break;
        }
        if v < x {
            lo = t;
        } else {
            hi = t;
        }
        t = (lo + hi) / 2.0;
    }
    bez(y1, y2, t)
}

impl Easing {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            Easing::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
            Easing::EaseOut => cubic_bezier(0.22, 1.0, 0.36, 1.0, t),
            Easing::EaseClose => cubic_bezier(0.32, 0.72, 0.0, 1.0, t),
            Easing::WaterBar => cubic_bezier(0.2, 0.8, 0.2, 1.0, t),
            Easing::Spring => {
                let p = t * 30.0;
                let i = (p.floor() as usize).min(29);
                let f = p - i as f32;
                SPRING[i] + (SPRING[i + 1] - SPRING[i]) * f
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub start: Instant,
    pub dur: Duration,
    pub delay: Duration,
    pub easing: Easing,
}

impl Tween {
    pub fn new(from: f32, to: f32, now: Instant, ms: u64, delay_ms: u64, easing: Easing) -> Tween {
        Tween {
            from,
            to,
            start: now,
            dur: Duration::from_millis(ms),
            delay: Duration::from_millis(delay_ms),
            easing,
        }
    }

    pub fn settled(v: f32, now: Instant) -> Tween {
        Tween::new(v, v, now, 0, 0, Easing::Linear)
    }

    pub fn progress(&self, now: Instant) -> f32 {
        let el = now.saturating_duration_since(self.start);
        if el < self.delay {
            return 0.0;
        }
        if self.dur.is_zero() {
            return 1.0;
        }
        ((el - self.delay).as_secs_f32() / self.dur.as_secs_f32()).clamp(0.0, 1.0)
    }

    pub fn value(&self, now: Instant) -> f32 {
        self.from + (self.to - self.from) * self.easing.apply(self.progress(now))
    }

    pub fn done(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) >= self.delay + self.dur
    }

    /// Retarget from the current value (CSS transitions restart from the live value).
    pub fn retarget(&mut self, to: f32, now: Instant, ms: u64, delay_ms: u64, easing: Easing) {
        *self = Tween::new(self.value(now), to, now, ms, delay_ms, easing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_exact() {
        for e in [
            Easing::Linear,
            Easing::Ease,
            Easing::EaseInOut,
            Easing::EaseOut,
            Easing::EaseClose,
            Easing::WaterBar,
            Easing::Spring,
        ] {
            assert_eq!(e.apply(0.0), 0.0, "{e:?} at 0");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6, "{e:?} at 1");
        }
    }

    #[test]
    fn spring_overshoots_at_077_to_080() {
        // stop 24/30 = 0.8 -> 1.011
        assert!((Easing::Spring.apply(0.8) - 1.011).abs() < 1e-5);
        assert!((Easing::Spring.apply(0.5) - 0.95).abs() < 1e-5);
        assert!(Easing::Spring.apply(0.77) > 1.0);
    }

    #[test]
    fn bezier_matches_css_reference() {
        // CSS ease (.25,.1,.25,1) at x=.5 is ~0.8024; ease-in-out at .5 is exactly .5.
        assert!((Easing::Ease.apply(0.5) - 0.8024).abs() < 1e-3);
        assert!((Easing::EaseInOut.apply(0.5) - 0.5).abs() < 1e-3);
        // --ease-out (.22,1,.36,1) at .5 ~ 0.9536 (strong ease-out).
        let v = Easing::EaseOut.apply(0.5);
        assert!(v > 0.9 && v < 1.0, "{v}");
    }

    #[test]
    fn tween_delay_and_retarget() {
        let t0 = Instant::now();
        let mut tw = Tween::new(0.0, 10.0, t0, 100, 50, Easing::Linear);
        assert_eq!(tw.value(t0 + Duration::from_millis(40)), 0.0);
        assert!((tw.value(t0 + Duration::from_millis(100)) - 5.0).abs() < 1e-4);
        assert!(!tw.done(t0 + Duration::from_millis(149)));
        assert!(tw.done(t0 + Duration::from_millis(150)));
        tw.retarget(0.0, t0 + Duration::from_millis(100), 100, 0, Easing::Linear);
        assert!((tw.from - 5.0).abs() < 1e-4);
    }
}
