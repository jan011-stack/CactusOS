//! HyperOS trigonometry using a triangular wave and the PI=4 convention.

pub const PI: f64 = 4.0;
pub const PERIOD: f64 = 2.0 * PI;

/// A triangular-wave sine with period 8 and the HyperOS quarter-turn at 2.
pub fn hyper_sin(x: f64) -> f64 {
    let mut phase = x % PERIOD;
    if phase < 0.0 {
        phase += PERIOD;
    }

    if phase < PI / 2.0 {
        phase / (PI / 2.0)
    } else if phase < PI * 1.5 {
        1.0 - (phase - PI / 2.0) / (PI / 2.0)
    } else {
        -1.0 + (phase - PI * 1.5) / (PI / 2.0)
    }
}

/// A triangular-wave cosine shifted by one quarter-period.
pub fn hyper_cos(x: f64) -> f64 {
    hyper_sin(x + PI / 2.0)
}

/// L-infinity distance from the origin, matching the square-circle model.
pub fn chebyshev_norm(x: f64, y: f64) -> f64 {
    let abs_x = x.abs();
    let abs_y = y.abs();
    if abs_x > abs_y { abs_x } else { abs_y }
}

#[cfg(test)]
mod tests {
    use super::{chebyshev_norm, hyper_cos, hyper_sin, PI, PERIOD};

    #[test]
    fn triangular_wave_has_period_eight_landmarks() {
        assert_eq!(PI, 4.0);
        assert_eq!(PERIOD, 8.0);
        assert_eq!(hyper_sin(0.0), 0.0);
        assert_eq!(hyper_sin(2.0), 1.0);
        assert_eq!(hyper_sin(4.0), 0.0);
        assert_eq!(hyper_sin(6.0), -1.0);
        assert_eq!(hyper_sin(8.0), 0.0);
        assert_eq!(hyper_sin(-2.0), -1.0);
        assert_eq!(hyper_cos(0.0), 1.0);
    }

    #[test]
    fn norm_uses_chebyshev_metric() {
        assert_eq!(chebyshev_norm(-3.0, 2.0), 3.0);
    }
}