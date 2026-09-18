//! Pure arc math shared by the usage ring and, later, the sunburst.
//! Angles are radians; 0 points right, positive sweeps clockwise on screen.

use std::f64::consts::PI;

pub fn fraction(used: u64, size: u64) -> f64 {
    if size == 0 {
        0.0
    } else {
        (used as f64 / size as f64).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Normal,
    Warning,
    Critical,
}

pub fn level(fraction: f64) -> Level {
    if fraction > 0.95 {
        Level::Critical
    } else if fraction > 0.85 {
        Level::Warning
    } else {
        Level::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub start: f64,
    pub end: f64,
}

/// An arc from twelve o'clock covering `fraction` of the circle, clockwise.
pub fn arc_for(fraction: f64) -> Arc {
    let start = -PI / 2.0;
    Arc {
        start,
        end: start + fraction.clamp(0.0, 1.0) * 2.0 * PI,
    }
}

pub fn point(cx: f64, cy: f64, radius: f64, angle: f64) -> (f64, f64) {
    (cx + radius * angle.cos(), cy + radius * angle.sin())
}

/// SVG large-arc flag for an arc covering `fraction` of the circle.
pub fn large_arc(fraction: f64) -> bool {
    fraction > 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn fraction_is_used_over_size_clamped() {
        assert_eq!(fraction(0, 0), 0.0);
        assert_eq!(fraction(50, 100), 0.5);
        assert_eq!(fraction(200, 100), 1.0);
    }

    #[test]
    fn levels_switch_above_85_and_95_percent() {
        assert_eq!(level(0.0), Level::Normal);
        assert_eq!(level(0.85), Level::Normal);
        assert_eq!(level(0.86), Level::Warning);
        assert_eq!(level(0.95), Level::Warning);
        assert_eq!(level(0.96), Level::Critical);
        assert_eq!(level(1.0), Level::Critical);
    }

    #[test]
    fn arcs_start_at_twelve_oclock_and_sweep_clockwise() {
        let quarter = arc_for(0.25);
        assert!(close(quarter.start, -PI / 2.0));
        assert!(close(quarter.end, 0.0));
        let full = arc_for(1.0);
        assert!(close(full.end - full.start, 2.0 * PI));
    }

    #[test]
    fn point_on_circle() {
        let (x, y) = point(10.0, 10.0, 5.0, -PI / 2.0);
        assert!(close(x, 10.0));
        assert!(close(y, 5.0));
    }

    #[test]
    fn large_arc_flag_flips_past_half() {
        assert!(!large_arc(0.5));
        assert!(large_arc(0.51));
    }
}
