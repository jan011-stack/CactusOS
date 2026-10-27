//! Integrity gate for staged data updates; never executes remote code.

use core::sync::atomic::{AtomicU64, Ordering};
use sha2::{Digest, Sha256};

use crate::hyper_euler_engine::{compute_euler_rotation, is_orthogonal_rotation, orthogonal_phase};
use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

pub const BLACKLIST_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateKind {
    Data,
    ProtectedData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardError {
    HashMismatch,
    InvalidIdentity,
    RootTokenRequired,
    ShadowTooSmall,
    SourceBlacklisted,
}

pub struct HyperChronoGuard {
    blacklist: [u32; BLACKLIST_CAPACITY],
    blacklist_len: usize,
    generation: AtomicU64,
}

impl HyperChronoGuard {
    pub const fn new() -> Self {
        Self {
            blacklist: [0; BLACKLIST_CAPACITY],
            blacklist_len: 0,
            generation: AtomicU64::new(0),
        }
    }

    pub fn is_blacklisted(&self, source: u32) -> bool {
        self.blacklist[..self.blacklist_len].contains(&source)
    }

    pub fn blacklist_source(&mut self, source: u32) {
        if !self.is_blacklisted(source) && self.blacklist_len < BLACKLIST_CAPACITY {
            self.blacklist[self.blacklist_len] = source;
            self.blacklist_len += 1;
        }
    }

    pub fn reject_source(&mut self, source: u32, shadow: &mut [u8]) {
        purge(shadow);
        self.blacklist_source(source);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Verify and stage data in caller-owned shadow storage.
    /// Protected data requires the configured root digest. No executable bytes
    /// are invoked or installed as function pointers by this guard.
    pub fn verify_and_stage(
        &mut self,
        terminal: &HyperSpaceTerminal,
        source: u32,
        kind: UpdateKind,
        expected_hash: &[u8; VECTOR_BYTES],
        payload: &[u8],
        root_token: Option<&[u8; VECTOR_BYTES]>,
        local_snapshot: &[u8; VECTOR_BYTES],
        shadow: &mut [u8],
    ) -> Result<[u8; VECTOR_BYTES], GuardError> {
        if self.is_blacklisted(source) {
            return Err(GuardError::SourceBlacklisted);
        }
        if payload.len() > shadow.len() {
            self.reject_and_blacklist(source, shadow);
            return Err(GuardError::ShadowTooSmall);
        }

        let digest = Sha256::digest(payload);
        let mut difference = 0u8;
        for index in 0..VECTOR_BYTES {
            difference |= digest[index] ^ expected_hash[index];
        }
        if difference != 0 {
            self.reject_and_blacklist(source, shadow);
            return Err(GuardError::HashMismatch);
        }

        let phase = orthogonal_phase(digest[0] & 3);
        if !is_orthogonal_rotation(compute_euler_rotation(phase)) {
            self.reject_and_blacklist(source, shadow);
            return Err(GuardError::InvalidIdentity);
        }
        if kind == UpdateKind::ProtectedData
            && !root_token.is_some_and(|token| terminal.verify_root_token(token))
        {
            self.reject_and_blacklist(source, shadow);
            return Err(GuardError::RootTokenRequired);
        }

        shadow.fill(0);
        shadow[..payload.len()].copy_from_slice(payload);
        let mut feature_delta = [0u8; VECTOR_BYTES];
        for index in 0..VECTOR_BYTES {
            feature_delta[index] = digest[index] ^ local_snapshot[index];
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(feature_delta)
    }

    fn reject_and_blacklist(&mut self, source: u32, shadow: &mut [u8]) {
        self.reject_source(source, shadow);
    }

}

impl Default for HyperChronoGuard {
    fn default() -> Self {
        Self::new()
    }
}

fn purge(memory: &mut [u8]) {
    for byte in memory {
        // SAFETY: exclusive access to the supplied shadow buffer is held.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}

#[cfg(test)]
mod tests {
    use super::{GuardError, HyperChronoGuard, UpdateKind};
    use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};
    use sha2::{Digest, Sha256};

    #[test]
    fn verified_data_is_staged_and_xor_compared_with_log_snapshot() {
        let terminal = HyperSpaceTerminal::new();
        let mut guard = HyperChronoGuard::new();
        let payload = b"data-only patch";
        let digest: [u8; VECTOR_BYTES] = Sha256::digest(payload).into();
        let local = [0x55; VECTOR_BYTES];
        let mut shadow = [0xaa; 32];
        let delta = guard
            .verify_and_stage(
                &terminal,
                42,
                UpdateKind::Data,
                &digest,
                payload,
                None,
                &local,
                &mut shadow,
            )
            .unwrap();
        assert_eq!(&shadow[..payload.len()], payload);
        assert!(shadow[payload.len()..].iter().all(|byte| *byte == 0));
        assert_eq!(delta[0], digest[0] ^ local[0]);
        assert_eq!(guard.generation(), 1);
    }

    #[test]
    fn invalid_digest_purges_shadow_and_blacklists_source() {
        let terminal = HyperSpaceTerminal::new();
        let mut guard = HyperChronoGuard::new();
        let mut shadow = [0xaa; 16];
        let result = guard.verify_and_stage(
            &terminal,
            7,
            UpdateKind::Data,
            &[0; VECTOR_BYTES],
            b"wrong hash",
            None,
            &[0; VECTOR_BYTES],
            &mut shadow,
        );
        assert_eq!(result, Err(GuardError::HashMismatch));
        assert_eq!(shadow, [0; 16]);
        assert!(guard.is_blacklisted(7));
    }
}