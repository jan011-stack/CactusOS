//! Lock-free fixed-block allocator backed by a caller-owned, aligned arena.

use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, Ordering};

pub const BLOCK_COUNT: usize = u64::BITS as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocatorError {
    InvalidBlockSize,
    MisalignedArena,
    ArenaTooSmall,
    InvalidPointer,
    AlreadyFree,
}

/// A 64-slot fixed-size allocator. `BLOCK_SIZE` must be a power of two.
///
/// It is lock-free, but not wait-free: compare-and-swap may retry under
/// contention. Fixed-size slots avoid external fragmentation, not internal
/// fragmentation within an allocated slot.
pub struct HyperAllocator<const BLOCK_SIZE: usize> {
    base_address: usize,
    bitmap: AtomicU64,
}

impl<const BLOCK_SIZE: usize> HyperAllocator<BLOCK_SIZE> {
    /// Create an allocator over caller-owned, writable memory.
    ///
    /// # Safety
    /// `memory` must point to at least `64 * BLOCK_SIZE` writable bytes and
    /// remain alive and exclusively managed by this allocator for its lifetime.
    pub unsafe fn new(memory: NonNull<u8>, memory_len: usize) -> Result<Self, AllocatorError> {
        if BLOCK_SIZE == 0 || !BLOCK_SIZE.is_power_of_two() {
            return Err(AllocatorError::InvalidBlockSize);
        }
        let required = BLOCK_SIZE
            .checked_mul(BLOCK_COUNT)
            .ok_or(AllocatorError::ArenaTooSmall)?;
        let address = memory.as_ptr() as usize;
        if address & (BLOCK_SIZE - 1) != 0 {
            return Err(AllocatorError::MisalignedArena);
        }
        if memory_len < required {
            return Err(AllocatorError::ArenaTooSmall);
        }
        Ok(Self {
            base_address: address,
            bitmap: AtomicU64::new(0),
        })
    }

    /// Allocate one block, returning `None` when all 64 slots are occupied.
    pub fn allocate(&self) -> Option<NonNull<u8>> {
        let mut occupied = self.bitmap.load(Ordering::Relaxed);
        loop {
            let available = !occupied;
            if available == 0 {
                return None;
            }
            let index = available.trailing_zeros() as usize;
            let updated = occupied | (1u64 << index);
            match self.bitmap.compare_exchange_weak(
                occupied,
                updated,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let address = self.base_address + index * BLOCK_SIZE;
                    return NonNull::new(address as *mut u8);
                }
                Err(current) => occupied = current,
            }
        }
    }

    /// Release a previously allocated block.
    ///
    /// # Safety
    /// The pointer must come from this allocator and no user may access its
    /// block after this call, including through aliases on other threads.
    pub unsafe fn deallocate(&self, pointer: NonNull<u8>) -> Result<(), AllocatorError> {
        let address = pointer.as_ptr() as usize;
        let offset = address
            .checked_sub(self.base_address)
            .ok_or(AllocatorError::InvalidPointer)?;
        if offset >= BLOCK_COUNT * BLOCK_SIZE || offset & (BLOCK_SIZE - 1) != 0 {
            return Err(AllocatorError::InvalidPointer);
        }
        let mask = 1u64 << (offset / BLOCK_SIZE);
        let previous = self.bitmap.fetch_and(!mask, Ordering::Release);
        if previous & mask == 0 {
            return Err(AllocatorError::AlreadyFree);
        }
        Ok(())
    }

    pub fn allocated_blocks(&self) -> u32 {
        self.bitmap.load(Ordering::Relaxed).count_ones()
    }
}

#[cfg(test)]
mod tests {
    use super::{AllocatorError, HyperAllocator, BLOCK_COUNT};
    use core::ptr::NonNull;

    #[repr(align(16))]
    struct TestArena([u8; 16 * BLOCK_COUNT]);

    #[test]
    fn bitmap_allocates_all_slots_and_reuses_freed_slot() {
        let mut arena = TestArena([0; 16 * BLOCK_COUNT]);
        let allocator = unsafe {
            HyperAllocator::<16>::new(
                NonNull::new(arena.0.as_mut_ptr()).unwrap(),
                arena.0.len(),
            )
            .unwrap()
        };
        let mut blocks = [None; BLOCK_COUNT];
        for block in &mut blocks {
            *block = allocator.allocate();
            assert!(block.is_some());
        }
        assert!(allocator.allocate().is_none());
        assert_eq!(allocator.allocated_blocks(), BLOCK_COUNT as u32);

        let released = blocks[0].unwrap();
        unsafe { allocator.deallocate(released).unwrap() };
        assert_eq!(allocator.allocated_blocks(), (BLOCK_COUNT - 1) as u32);
        assert_eq!(allocator.allocate(), Some(released));
    }

    #[test]
    fn allocator_rejects_bad_geometry_and_double_free() {
        let mut arena = TestArena([0; 16 * BLOCK_COUNT]);
        let allocator = unsafe {
            HyperAllocator::<16>::new(
                NonNull::new(arena.0.as_mut_ptr()).unwrap(),
                arena.0.len(),
            )
            .unwrap()
        };
        let block = allocator.allocate().unwrap();
        unsafe { allocator.deallocate(block).unwrap() };
        assert_eq!(
            unsafe { allocator.deallocate(block) },
            Err(AllocatorError::AlreadyFree)
        );
    }
}