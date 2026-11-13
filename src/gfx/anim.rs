//! Easing curves and tweens matching the CSS transitions of the old UI.

#[derive(Clone, Debug, PartialEq)]
pub enum Easing {
    Linear,
    Bezier(f32, f32, f32, f32),
    /// `steps(n, start|end)`; `true` = jump-start.
    Steps(u32, bool),
    /// CSS `linear()` stops `(input, output)`, inputs ascending in 0..=1.
    Points(Vec<(f32, f32)>),
    /// Damped spring with SwiftUI semantics: the transition duration is the response
    /// (perceptual duration), the payload the bounce: `0` critically damped, up to `0.9`.
    Spring(f32),
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

/// Angular frequency and damping ratio of a spring whose time unit is its response.
fn spring_terms(bounce: f32) -> (f32, f32) {
    (std::f32::consts::TAU, 1.0 - bounce.clamp(0.0, 0.9))
}

/// Unit spring travelling 0 → 1 that starts with velocity `v0`: `(position, velocity)`
/// after `t` responses, velocity per response.
pub fn spring(bounce: f32, v0: f32, t: f32) -> (f32, f32) {
    let (w, z) = spring_terms(bounce);
    let decay = (-z * w * t).exp();
    if z >= 1.0 {
        let b = v0 - w;
        (1.0 + decay * (b * t - 1.0), decay * (b - w * (b * t - 1.0)))
    } else {
        let wd = w * (1.0 - z * z).sqrt();
        let b = (v0 - z * w) / wd;
        let (s, c) = (wd * t).sin_cos();
        (1.0 + decay * (b * s - c), decay * ((wd * b + z * w) * c + (wd - z * w * b) * s))
    }
}

/// Responses until that spring stays within 0.05 % of its target.
pub fn spring_settle(bounce: f32, v0: f32) -> f32 {
    const EPS: f32 = 0.0005;
    let (w, z) = spring_terms(bounce);
    if z >= 1.0 {
        let b = (v0 - w).abs();
        let mut t = (1.0 / EPS).ln() / w;
        for _ in 0..3 {
            t = ((1.0 + b * t) / EPS).ln() / w;
        }
        t
    } else {
        let wd = w * (1.0 - z * z).sqrt();
        (1.0f32.hypot((v0 - z * w) / wd) / EPS).ln() / (z * w)
    }
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
            // Outside a transition (keyframes) the whole settle maps onto 0..1.
            &Easing::Spring(b) => if t >= 1.0 { 1.0 } else { spring(b, 0.0, t * spring_settle(b, 0.0)).0 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sampled_spring() -> Easing {
        let ys = [
            0.0, 0.024, 0.0842, 0.1666, 0.2603, 0.3575, 0.4528, 0.5424, 0.6241, 0.6968, 0.76, 0.8139, 0.8591, 0.8962,
            0.9262, 0.95, 0.9685, 0.9825, 0.9929, 1.0002, 1.0053, 1.0085, 1.0102, 1.011, 1.011, 1.0106, 1.0098, 1.0088,
            1.0077, 1.0066, 1.0,
        ];
        Easing::Points(ys.iter().enumerate().map(|(i, &y)| (i as f32 / 30.0, y)).collect())
    }

    #[test]
    fn endpoints_exact() {
        for e in [Easing::Linear, EASE, Easing::Bezier(0.22, 1.0, 0.36, 1.0), Easing::Steps(2, true), sampled_spring(), Easing::Spring(0.2), Easing::Spring(0.0)] {
            assert_eq!(e.apply(0.0) == 0.0 || matches!(e, Easing::Steps(_, true)), true, "{e:?} at 0");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-6, "{e:?} at 1");
        }
    }

    #[test]
    fn spring_and_steps() {
        assert!((sampled_spring().apply(0.8) - 1.011).abs() < 1e-3);
        assert!(sampled_spring().apply(0.77) > 1.0);
        assert_eq!(Easing::Steps(2, true).apply(0.1), 0.5);
        assert_eq!(Easing::Steps(2, true).apply(0.6), 1.0);
        assert_eq!(Easing::Steps(2, false).apply(0.6), 0.5);
    }

    #[test]
    fn bezier_matches_css_reference() {
        assert!((EASE.apply(0.5) - 0.8024).abs() < 1e-3);
        assert!((Easing::Bezier(0.42, 0.0, 0.58, 1.0).apply(0.5) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn spring_matches_damped_oscillator() {
        // Starts at rest on 0 and honours the initial velocity.
        let (x, v) = spring(0.2, 0.0, 0.0);
        assert!(x == 0.0 && v.abs() < 1e-4, "{x} {v}");
        assert!((spring(0.2, 3.0, 0.0).1 - 3.0).abs() < 1e-4);
        assert!((spring(0.0, -3.0, 0.0).1 + 3.0).abs() < 1e-4);
        // Velocity is the derivative of position.
        for (b, v0, t) in [(0.2, 0.0, 0.3), (0.2, -4.0, 0.7), (0.0, 0.0, 0.4), (0.0, 5.0, 0.2)] {
            let h = 1e-3;
            let slope = (spring(b, v0, t + h).0 - spring(b, v0, t - h).0) / (2.0 * h);
            assert!((slope - spring(b, v0, t).1).abs() < 2e-2, "bounce {b} v0 {v0} t {t}: {slope}");
        }
        // Damping 0.8 overshoots by e^(-0.8π/0.6) ≈ 1.5 %; critical damping never overshoots.
        let peak = |b: f32| (0..400).map(|i| spring(b, 0.0, i as f32 / 100.0).0).fold(0.0f32, f32::max);
        assert!((peak(0.2) - 1.0152).abs() < 1e-3, "{}", peak(0.2));
        assert!(peak(0.0) <= 1.0);
        // Settled means the remaining travel is invisible, with or without carried velocity.
        for (b, v0) in [(0.2, 0.0), (0.2, -20.0), (0.0, 0.0), (0.0, 20.0)] {
            let end = spring_settle(b, v0);
            assert!((spring(b, v0, end).0 - 1.0).abs() <= 6e-4, "bounce {b} v0 {v0}");
            assert!(end < 3.0, "bounce {b} v0 {v0}: {end}");
        }
        assert!((Easing::Spring(0.2).apply(1.0) - 1.0).abs() < 1e-6);
        assert_eq!(Easing::Spring(0.2).apply(0.0), 0.0);
    }
}
