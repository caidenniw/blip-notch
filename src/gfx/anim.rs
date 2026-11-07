//! Easing curves and tweens matching the CSS transitions of the old UI.

#[derive(Clone, Debug, PartialEq)]
pub enum Easing {
    Linear,
    Bezier(f32, f32, f32, f32),
    /// `steps(n, start|end)`; `true` = jump-start.
    Steps(u32, bool),
    /// CSS `linear()` stops `(input, output)`, inputs ascending in 0..=1.
    Points(Vec<(f32, f32)>),
}

pub const EASE: Easing = Easing::Bezier(0.25, 0.1, 0.25, 1.0);

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
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            &Easing::Bezier(a, b, c, d) => cubic_bezier(a, b, c, d, t),
            &Easing::Steps(n, start) => {
                let n = n.max(1) as f32;
                let s = if start { (t * n).floor() + 1.0 } else { (t * n).floor() };
                if t >= 1.0 { 1.0 } else { (s / n).min(1.0) }
            }
            Easing::Points(p) => {
                if p.is_empty() {
                    return t;
                }
                let i = p.iter().position(|&(x, _)| x >= t).unwrap_or(p.len() - 1);
                if i == 0 {
                    return p[0].1;
                }
                let (x0, y0) = p[i - 1];
                let (x1, y1) = p[i];
                if x1 <= x0 { y1 } else { y0 + (y1 - y0) * (t - x0) / (x1 - x0) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spring() -> Easing {
        let ys = [
            0.0, 0.024, 0.0842, 0.1666, 0.2603, 0.3575, 0.4528, 0.5424, 0.6241, 0.6968, 0.76, 0.8139, 0.8591, 0.8962,
            0.9262, 0.95, 0.9685, 0.9825, 0.9929, 1.0002, 1.0053, 1.0085, 1.0102, 1.011, 1.011, 1.0106, 1.0098, 1.0088,
            1.0077, 1.0066, 1.0,
        ];
        Easing::Points(ys.iter().enumerate().map(|(i, &y)| (i as f32 / 30.0, y)).collect())
    }

    #[test]
    fn endpoints_exact() {
        for e in [Easing::Linear, EASE, Easing::Bezier(0.22, 1.0, 0.36, 1.0), Easing::Steps(2, true), spring()] {
            assert_eq!(e.apply(0.0) == 0.0 || matches!(e, Easing::Steps(_, true)), true, "{e:?} at 0");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6, "{e:?} at 1");
        }
    }

    #[test]
    fn spring_and_steps() {
        assert!((spring().apply(0.8) - 1.011).abs() < 1e-3);
        assert!(spring().apply(0.77) > 1.0);
        assert_eq!(Easing::Steps(2, true).apply(0.1), 0.5);
        assert_eq!(Easing::Steps(2, true).apply(0.6), 1.0);
        assert_eq!(Easing::Steps(2, false).apply(0.6), 0.5);
    }

    #[test]
    fn bezier_matches_css_reference() {
        assert!((EASE.apply(0.5) - 0.8024).abs() < 1e-3);
        assert!((Easing::Bezier(0.42, 0.0, 0.58, 1.0).apply(0.5) - 0.5).abs() < 1e-3);
    }
}
