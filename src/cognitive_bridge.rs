//! Four fixed agent context slots with a token-gated link and flag-checked broadcast.

use core::cell::UnsafeCell;
use core::ptr::write_volatile;

pub const ID_ALEX: u8 = 0x01;
pub const ID_ASTYRIA: u8 = 0x02;
pub const ID_AURELIUS: u8 = 0x03;
pub const ID_SI_1: u8 = 0x04;

pub const CONTEXT_BUFFER_SIZE: usize = 4096;
pub const LINK_ACTIVE: u8 = 0b1000_0000;

const fn padded(text: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut index = 0;
    while index < text.len() {
        out[index] = text[index];
        index += 1;
    }
    out
}

/// The token is public in the source, so this gates the API but is not a secret.
pub const LINK_TOKEN: [u8; 32] = padded(b"364_OMNIKLUCZ_ROOT");

#[repr(C, align(64))]
pub struct CognitiveContextSlot {
    pub agent_id: u8,
    pub bitmask_flag: u8,
    pub memory_boundary_start: usize,
    pub memory_boundary_end: usize,
    length: usize,
    buffer: [u8; CONTEXT_BUFFER_SIZE],
}

impl CognitiveContextSlot {
    const fn new(agent_id: u8, bitmask_flag: u8, start: usize, end: usize) -> Self {
        Self {
            agent_id,
            bitmask_flag,
            memory_boundary_start: start,
            memory_boundary_end: end,
            length: 0,
            buffer: [0; CONTEXT_BUFFER_SIZE],
        }
    }

    pub fn is_linked(&self) -> bool {
        self.bitmask_flag & LINK_ACTIVE != 0
    }

    /// The last payload written to this slot.
    pub fn message(&self) -> &[u8] {
        &self.buffer[..self.length]
    }

    fn write(&mut self, payload: &[u8]) {
        for (index, &byte) in payload.iter().enumerate() {
            // SAFETY: `index < payload.len() <= CONTEXT_BUFFER_SIZE`, checked by callers.
            unsafe { write_volatile(self.buffer.as_mut_ptr().add(index), byte) };
        }
        self.length = payload.len();
    }
}

pub struct CognitiveBridge {
    slots: [CognitiveContextSlot; 4],
}

impl CognitiveBridge {
    pub const fn new() -> Self {
        Self {
            slots: [
                CognitiveContextSlot::new(ID_ALEX, 0b0001, 0xFAAA_0000, 0xFAAA_FFFF),
                CognitiveContextSlot::new(ID_ASTYRIA, 0b0010, 0xFBBB_0000, 0xFBBB_FFFF),
                CognitiveContextSlot::new(ID_AURELIUS, 0b0100, 0xFCCC_0000, 0xFCCC_FFFF),
                CognitiveContextSlot::new(ID_SI_1, 0b1000, 0xFDDD_0000, 0xFDDD_FFFF),
            ],
        }
    }

    pub fn slot(&self, agent_id: u8) -> Option<&CognitiveContextSlot> {
        self.slots.iter().find(|slot| slot.agent_id == agent_id)
    }

    /// A wrong token revokes the link on every slot but keeps the identity bits.
    pub fn establish_link(&mut self, token: &[u8; 32]) -> bool {
        let mut difference = 0u8;
        for index in 0..32 {
            difference |= token[index] ^ LINK_TOKEN[index];
        }
        let accepted = difference == 0;
        for slot in self.slots.iter_mut() {
            let flags = if accepted {
                slot.bitmask_flag | LINK_ACTIVE
            } else {
                slot.bitmask_flag & !LINK_ACTIVE
            };
            // SAFETY: `slot` is an exclusive borrow of a valid field.
            unsafe { write_volatile(&mut slot.bitmask_flag, flags) };
        }
        accepted
    }

    /// Writes the payload to every linked slot and returns how many were written.
    pub fn broadcast(&mut self, payload: &[u8]) -> usize {
        if payload.len() > CONTEXT_BUFFER_SIZE {
            return 0;
        }
        let mut written = 0;
        for slot in self.slots.iter_mut().filter(|slot| slot.is_linked()) {
            slot.write(payload);
            written += 1;
        }
        written
    }

    /// Writes the payload to one linked slot.
    pub fn send_to(&mut self, agent_id: u8, payload: &[u8]) -> bool {
        if payload.len() > CONTEXT_BUFFER_SIZE {
            return false;
        }
        match self
            .slots
            .iter_mut()
            .find(|slot| slot.agent_id == agent_id && slot.is_linked())
        {
            Some(slot) => {
                slot.write(payload);
                true
            }
            None => false,
        }
    }
}

impl Default for CognitiveBridge {
    fn default() -> Self {
        Self::new()
    }
}

pub struct GlobalBridge(UnsafeCell<CognitiveBridge>);

// SAFETY: the C entry points below are documented as single-threaded.
unsafe impl Sync for GlobalBridge {}

/// Static storage for the four 4 KiB agent slots, reserved in .bss.
pub static COGNITIVE_BRIDGE_MATRIX: GlobalBridge =
    GlobalBridge(UnsafeCell::new(CognitiveBridge::new()));

/// # Safety
/// `root_token` must point to 32 readable bytes and calls must not run concurrently.
#[no_mangle]
pub unsafe extern "C" fn establish_cognitive_link(root_token: *const u8) -> bool {
    if root_token.is_null() {
        return false;
    }
    let token = &*(root_token as *const [u8; 32]);
    (*COGNITIVE_BRIDGE_MATRIX.0.get()).establish_link(token)
}

/// # Safety
/// `payload` must point to `length` readable bytes and calls must not run concurrently.
#[no_mangle]
pub unsafe extern "C" fn broadcast_to_agents(payload: *const u8, length: usize) -> usize {
    let bytes: &[u8] = if length == 0 {
        &[]
    } else if payload.is_null() {
        return 0;
    } else {
        core::slice::from_raw_parts(payload, length)
    };
    (*COGNITIVE_BRIDGE_MATRIX.0.get()).broadcast(bytes)
}

#[cfg(test)]
mod tests {
    use super::{CognitiveBridge, CONTEXT_BUFFER_SIZE, ID_ALEX, ID_SI_1, LINK_TOKEN};

    #[test]
    fn broadcast_reaches_only_linked_slots_after_correct_token() {
        let mut bridge = CognitiveBridge::new();
        assert_eq!(bridge.broadcast(b"hej"), 0);
        assert!(bridge.establish_link(&LINK_TOKEN));
        assert_eq!(bridge.broadcast(b"hej"), 4);
        assert_eq!(bridge.slot(ID_SI_1).unwrap().message(), b"hej");
    }

    #[test]
    fn wrong_token_revokes_link_but_keeps_identity_bits() {
        let mut bridge = CognitiveBridge::new();
        assert!(bridge.establish_link(&LINK_TOKEN));
        assert!(!bridge.establish_link(&[0; 32]));
        assert_eq!(bridge.slot(ID_ALEX).unwrap().bitmask_flag, 0b0001);
        assert_eq!(bridge.broadcast(b"x"), 0);
        assert!(bridge.establish_link(&LINK_TOKEN));
        assert!(bridge.send_to(ID_ALEX, b"tylko alex"));
        assert!(bridge.slot(ID_SI_1).unwrap().message().is_empty());
    }

    #[test]
    fn matrix_reserves_four_disjoint_blocks() {
        use super::COGNITIVE_BRIDGE_MATRIX;
        assert!(core::mem::size_of_val(&COGNITIVE_BRIDGE_MATRIX) >= 4 * CONTEXT_BUFFER_SIZE);
        let bridge = CognitiveBridge::new();
        let ranges: [(usize, usize); 4] = core::array::from_fn(|index| {
            let slot = bridge.slot(index as u8 + 1).unwrap();
            (slot.memory_boundary_start, slot.memory_boundary_end)
        });
        for (index, (start, end)) in ranges.iter().enumerate() {
            assert!(start < end);
            assert!(ranges[index + 1..].iter().all(|(other, _)| other > end));
        }
    }

    #[test]
    fn oversized_payload_is_rejected() {
        let mut bridge = CognitiveBridge::new();
        bridge.establish_link(&LINK_TOKEN);
        assert_eq!(bridge.broadcast(&[0; CONTEXT_BUFFER_SIZE + 1]), 0);
        assert!(!bridge.send_to(ID_ALEX, &[0; CONTEXT_BUFFER_SIZE + 1]));
    }
}
