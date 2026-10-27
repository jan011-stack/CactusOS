//! Allocation-free power-on self-tests for phase geometry and security boundaries.

use crate::chrono_log::{ChronoLogEngine, LogEntry};
use crate::hyper_euler_engine::{compute_euler_rotation, is_orthogonal_rotation};
use crate::hyper_shield::{HyperShieldController, SecurityRing};
use crate::kernel::{ConfigError, HyperKernel};
use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

const TEST_CONTEXT_ID: u32 = 0x504f_5354;
const SHIELD_BREACH_MARKER: u8 = 0xb1;

/// Verify discrete Euler rotations at every orthogonal PI=4 grid boundary.
pub fn run_post_validation() -> bool {
    let phases = [0.0, 2.0, 4.0, 6.0, 8.0, -2.0, 10.0];
    for phase in phases {
        let rotation = compute_euler_rotation(phase);
        if !is_orthogonal_rotation(rotation) {
            return false;
        }
    }

    if is_orthogonal_rotation(compute_euler_rotation(1.0)) {
        return false;
    }

    test_shield_integrity()
}

/// Simulate an unauthorized Ring 0 write and verify purge, state lock, and logging.
pub fn test_shield_integrity() -> bool {
    let terminal = HyperSpaceTerminal::new();
    let shield = HyperShieldController::new();
    let mut kernel = HyperKernel::new();
    let mut malicious_payload = [0xa5; VECTOR_BYTES];

    let authorized = shield.authorize_or_purge(
        SecurityRing::Ring0,
        None,
        &mut malicious_payload,
    );
    let mutation = kernel.set_operator(&terminal, &[0; VECTOR_BYTES], "intruder");
    let payload_purged = malicious_payload.iter().all(|byte| *byte == 0);
    let identity_locked = kernel.operator() == "Mateusz Mitek";

    let log = ChronoLogEngine::new();
    let mut snapshot = [0; VECTOR_BYTES];
    snapshot[0] = SHIELD_BREACH_MARKER;
    snapshot[1] = u8::from(!authorized);
    snapshot[2] = u8::from(mutation == Err(ConfigError::InvalidRootToken));
    snapshot[3] = u8::from(payload_purged);
    snapshot[4] = u8::from(identity_locked);
    let logged = log
        .push(LogEntry {
            timestamp: 0,
            context_id: TEST_CONTEXT_ID,
            status_snapshot: snapshot,
        })
        .is_ok();
    let log_verified = log.pop().is_some_and(|entry| {
        entry.context_id == TEST_CONTEXT_ID
            && entry.status_snapshot[0] == SHIELD_BREACH_MARKER
            && entry.status_snapshot[1..5] == [1, 1, 1, 1]
    });

    !authorized
        && mutation == Err(ConfigError::InvalidRootToken)
        && payload_purged
        && identity_locked
        && logged
        && log_verified
}

#[cfg(test)]
mod tests {
    use super::{run_post_validation, test_shield_integrity};

    #[test]
    fn post_validates_euler_grid_and_shield_boundary() {
        assert!(run_post_validation());
    }

    #[test]
    fn invalid_operator_write_is_purged_and_logged() {
        assert!(test_shield_integrity());
    }
}