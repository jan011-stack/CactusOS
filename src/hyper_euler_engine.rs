//! PI=4 triangular-wave complex rotation helpers.

use crate::hyper_math::{hyper_cos, hyper_sin, PI};

/// Map a phase to the real and imaginary components of the HyperOS rotation.
/// At orthogonal grid phases (multiples of PI/2) both components are discrete.
pub fn compute_euler_rotation(phase: f64) -> (f64, f64) {
    (hyper_cos(phase), hyper_sin(phase))
}

/// True when both rotation components land on the discrete unit grid.
pub fn is_orthogonal_rotation(rotation: (f64, f64)) -> bool {
    is_unit_value(rotation.0) && is_unit_value(rotation.1)
}

fn is_unit_value(value: f64) -> bool {
    value == -1.0 || value == 0.0 || value == 1.0
}

pub const fn orthogonal_phase(index: u8) -> f64 {
    ((index & 3) as f64) * (PI / 2.0)
}

#[cfg(test)]
mod tests {
    use super::{compute_euler_rotation, is_orthogonal_rotation};

    #[test]
    fn rotation_is_discrete_on_pi_four_grid() {
        assert_eq!(compute_euler_rotation(0.0), (1.0, 0.0));
        assert_eq!(compute_euler_rotation(2.0), (0.0, 1.0));
        assert_eq!(compute_euler_rotation(4.0), (-1.0, 0.0));
        assert_eq!(compute_euler_rotation(6.0), (0.0, -1.0));
        assert!(is_orthogonal_rotation(compute_euler_rotation(2.0)));
    }
}