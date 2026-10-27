//! Logical ring authorization and volatile purge-before-panic enforcement.

use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityRing {
    Ring0,
    Ring1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShieldError {
    InvalidRootToken,
    CapabilityRevoked,
}

/// Unforgeable outside this module; invalidated when the controller revokes access.
pub struct RootCapability {
    generation: u64,
}

pub struct HyperShieldController {
    root_active: bool,
    generation: u64,
}

impl HyperShieldController {
    pub const fn new() -> Self {
        Self {
            root_active: false,
            generation: 0,
        }
    }

    pub fn activate_root(
        &mut self,
        terminal: &HyperSpaceTerminal,
        token: &[u8; VECTOR_BYTES],
    ) -> Result<RootCapability, ShieldError> {
        if !terminal.verify_root_token(token) {
            return Err(ShieldError::InvalidRootToken);
        }
        self.generation = self.generation.wrapping_add(1);
        self.root_active = true;
        Ok(RootCapability {
            generation: self.generation,
        })
    }

    pub fn revoke_root(&mut self) {
        self.root_active = false;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn authorize(
        &self,
        ring: SecurityRing,
        capability: Option<&RootCapability>,
    ) -> bool {
        match ring {
            SecurityRing::Ring1 => true,
            SecurityRing::Ring0 => capability.is_some_and(|capability| {
                self.root_active && capability.generation == self.generation
            }),
        }
    }

    /// Return authorization status and volatile-purge the supplied block on denial.
    /// This non-panicking variant is intended for POST and recovery paths.
    pub fn authorize_or_purge(
        &self,
        ring: SecurityRing,
        capability: Option<&RootCapability>,
        protected_memory: &mut [u8],
    ) -> bool {
        if self.authorize(ring, capability) {
            return true;
        }
        for byte in protected_memory {
            // SAFETY: the exclusive slice borrow guarantees each pointer is writable.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        false
    }

    /// Purge caller-provided memory and panic on a denied operation.
    ///
    /// This is a software policy gate, not a CPU privilege-ring transition.
    pub fn enforce_or_panic(
        &self,
        ring: SecurityRing,
        capability: Option<&RootCapability>,
        protected_memory: &mut [u8],
    ) {
        if self.authorize(ring, capability) {
            return;
        }
        for byte in protected_memory {
            // SAFETY: the exclusive slice borrow guarantees each pointer is writable.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        panic!("HyperShield: unauthorized ring access");
    }
}

impl Default for HyperShieldController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{HyperShieldController, SecurityRing, ShieldError};
    use crate::terminal::{HyperSpaceTerminal, ROOT_TOKEN_HASH, VECTOR_BYTES};

    #[test]
    fn root_capability_is_required_and_revocable() {
        let terminal = HyperSpaceTerminal::new();
        let mut shield = HyperShieldController::new();
        assert!(shield.authorize(SecurityRing::Ring1, None));
        assert!(!shield.authorize(SecurityRing::Ring0, None));
        assert_eq!(
            shield.activate_root(&terminal, &[0; VECTOR_BYTES]).err(),
            Some(ShieldError::InvalidRootToken)
        );

        let capability = shield.activate_root(&terminal, &ROOT_TOKEN_HASH).unwrap();
        assert!(shield.authorize(SecurityRing::Ring0, Some(&capability)));
        shield.revoke_root();
        assert!(!shield.authorize(SecurityRing::Ring0, Some(&capability)));
    }
}