//! Borrow-checked per-agent memory views and token-gated kernel writes.

use crate::hyper_agent_bridge::AgentIdentity;
use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

/// An agent's exclusive writable segment and read-only system views.
pub struct HyperAgentCompartment<'a> {
    identity: AgentIdentity,
    sector: &'a mut [u8],
    context_matrix: &'a [u8],
    kv_cache: &'a [u8],
}

impl<'a> HyperAgentCompartment<'a> {
    pub fn new(
        identity: AgentIdentity,
        sector: &'a mut [u8],
        context_matrix: &'a [u8],
        kv_cache: &'a [u8],
    ) -> Self {
        Self {
            identity,
            sector,
            context_matrix,
            kv_cache,
        }
    }

    pub const fn identity(&self) -> AgentIdentity {
        self.identity
    }

    pub fn context_matrix(&self) -> &[u8] {
        self.context_matrix
    }

    pub fn kv_cache(&self) -> &[u8] {
        self.kv_cache
    }

    pub fn sector(&self) -> &[u8] {
        self.sector
    }

    pub fn write_local(&mut self, offset: usize, data: &[u8]) {
        let Some(end) = offset.checked_add(data.len()) else {
            self.purge_and_panic();
        };
        if end > self.sector.len() {
            self.purge_and_panic();
        }
        self.sector[offset..end].copy_from_slice(data);
    }

    /// Write protected kernel state after verifying the configured root digest.
    /// On denial or out-of-range access, volatile-zeroes the provided kernel block and panics.
    pub fn write_kernel_state(
        &self,
        terminal: &HyperSpaceTerminal,
        token: &[u8; VECTOR_BYTES],
        kernel_block: &mut [u8],
        offset: usize,
        data: &[u8],
    ) {
        let end = offset
            .checked_add(data.len())
            .filter(|end| *end <= kernel_block.len());
        if !terminal.verify_root_token(token) {
            purge_and_panic(kernel_block);
        }
        let Some(end) = end else {
            purge_and_panic(kernel_block);
        };
        kernel_block[offset..end].copy_from_slice(data);
    }

    fn purge_and_panic(&mut self) -> ! {
        purge(self.sector);
        panic!("HyperAgentCompartment: write outside agent sector");
    }
}

fn purge(memory: &mut [u8]) {
    for byte in memory {
        // SAFETY: caller provides exclusive access to this memory slice.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}

fn purge_and_panic(memory: &mut [u8]) -> ! {
    purge(memory);
    panic!("HyperAgentCompartment: unauthorized kernel write");
}

#[cfg(test)]
mod tests {
    use super::HyperAgentCompartment;
    use crate::hyper_agent_bridge::AgentIdentity;
    use crate::terminal::{HyperSpaceTerminal, ROOT_TOKEN_HASH, VECTOR_BYTES};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn compartment_exposes_read_only_shared_views_and_local_writes() {
        let mut local = [0; 16];
        let context = [3; 8];
        let cache = [4; 12];
        let mut compartment =
            HyperAgentCompartment::new(AgentIdentity::Astyria, &mut local, &context, &cache);
        compartment.write_local(4, &[9, 8]);
        assert_eq!(compartment.identity(), AgentIdentity::Astyria);
        assert_eq!(compartment.context_matrix(), &[3; 8]);
        assert_eq!(compartment.kv_cache(), &[4; 12]);
        assert_eq!(&compartment.sector()[4..6], &[9, 8]);
    }

    #[test]
    fn invalid_root_write_purges_before_panicking() {
        let terminal = HyperSpaceTerminal::new();
        let mut local = [0; 8];
        let compartment = HyperAgentCompartment::new(AgentIdentity::Alex, &mut local, &[], &[]);
        let mut kernel = [0xa5; 8];
        let result = catch_unwind(AssertUnwindSafe(|| {
            compartment.write_kernel_state(
                &terminal,
                &[0; VECTOR_BYTES],
                &mut kernel,
                0,
                &[1],
            )
        }));
        assert!(result.is_err());
        assert_eq!(kernel, [0; 8]);
        compartment.write_kernel_state(&terminal, &ROOT_TOKEN_HASH, &mut kernel, 0, &[1]);
        assert_eq!(kernel[0], 1);
    }

    #[test]
    fn cross_sector_write_purges_local_segment_before_panicking() {
        let mut local = [0xa5; 8];
        let mut compartment = HyperAgentCompartment::new(AgentIdentity::Alex, &mut local, &[], &[]);
        let result = catch_unwind(AssertUnwindSafe(|| compartment.write_local(7, &[1, 2])));
        assert!(result.is_err());
        assert_eq!(compartment.sector(), &[0; 8]);
    }
}