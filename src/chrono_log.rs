//! Bounded lock-free MPMC ring log with fixed-size, allocation-free records.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const LOG_CAPACITY: usize = 64;
pub const LOG_INDEX_MASK: usize = LOG_CAPACITY - 1;
pub const STATUS_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogEntry {
    pub timestamp: u64,
    pub context_id: u32,
    pub status_snapshot: [u8; STATUS_BYTES],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogError {
    Full,
}

struct LogSlot {
    sequence: AtomicUsize,
    entry: UnsafeCell<MaybeUninit<LogEntry>>,
}

macro_rules! initial_log_slots {
    () => {
        [
            LogSlot::new(0), LogSlot::new(1), LogSlot::new(2), LogSlot::new(3),
            LogSlot::new(4), LogSlot::new(5), LogSlot::new(6), LogSlot::new(7),
            LogSlot::new(8), LogSlot::new(9), LogSlot::new(10), LogSlot::new(11),
            LogSlot::new(12), LogSlot::new(13), LogSlot::new(14), LogSlot::new(15),
            LogSlot::new(16), LogSlot::new(17), LogSlot::new(18), LogSlot::new(19),
            LogSlot::new(20), LogSlot::new(21), LogSlot::new(22), LogSlot::new(23),
            LogSlot::new(24), LogSlot::new(25), LogSlot::new(26), LogSlot::new(27),
            LogSlot::new(28), LogSlot::new(29), LogSlot::new(30), LogSlot::new(31),
            LogSlot::new(32), LogSlot::new(33), LogSlot::new(34), LogSlot::new(35),
            LogSlot::new(36), LogSlot::new(37), LogSlot::new(38), LogSlot::new(39),
            LogSlot::new(40), LogSlot::new(41), LogSlot::new(42), LogSlot::new(43),
            LogSlot::new(44), LogSlot::new(45), LogSlot::new(46), LogSlot::new(47),
            LogSlot::new(48), LogSlot::new(49), LogSlot::new(50), LogSlot::new(51),
            LogSlot::new(52), LogSlot::new(53), LogSlot::new(54), LogSlot::new(55),
            LogSlot::new(56), LogSlot::new(57), LogSlot::new(58), LogSlot::new(59),
            LogSlot::new(60), LogSlot::new(61), LogSlot::new(62), LogSlot::new(63),
        ]
    };
}

impl LogSlot {
    const fn new(sequence: usize) -> Self {
        Self {
            sequence: AtomicUsize::new(sequence),
            entry: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

// Slot sequence ownership ensures a writer and reader never access a record concurrently.
unsafe impl Sync for LogSlot {}

/// Fixed 8x8 queue. `push` returns `Full` instead of blocking when saturated.
pub struct ChronoLogEngine {
    slots: [LogSlot; LOG_CAPACITY],
    enqueue: AtomicUsize,
    dequeue: AtomicUsize,
}

impl ChronoLogEngine {
    pub const fn new() -> Self {
        Self {
            slots: initial_log_slots!(),
            enqueue: AtomicUsize::new(0),
            dequeue: AtomicUsize::new(0),
        }
    }

    pub fn push(&self, entry: LogEntry) -> Result<(), LogError> {
        let mut position = self.enqueue.load(Ordering::Relaxed);
        loop {
            let slot = &self.slots[position & LOG_INDEX_MASK];
            let sequence = slot.sequence.load(Ordering::Acquire);
            let difference = sequence.wrapping_sub(position) as isize;
            if difference == 0 {
                match self.enqueue.compare_exchange_weak(
                    position,
                    position.wrapping_add(1),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // SAFETY: this producer reserved the slot sequence exclusively.
                        unsafe { (*slot.entry.get()).write(entry) };
                        slot.sequence.store(position.wrapping_add(1), Ordering::Release);
                        return Ok(());
                    }
                    Err(current) => position = current,
                }
            } else if difference < 0 {
                return Err(LogError::Full);
            } else {
                position = self.enqueue.load(Ordering::Relaxed);
            }
        }
    }

    pub fn pop(&self) -> Option<LogEntry> {
        let mut position = self.dequeue.load(Ordering::Relaxed);
        loop {
            let slot = &self.slots[position & LOG_INDEX_MASK];
            let expected = position.wrapping_add(1);
            let sequence = slot.sequence.load(Ordering::Acquire);
            let difference = sequence.wrapping_sub(expected) as isize;
            if difference == 0 {
                match self.dequeue.compare_exchange_weak(
                    position,
                    expected,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // SAFETY: this consumer reserved the initialized slot exclusively.
                        let entry = unsafe { (*slot.entry.get()).assume_init_read() };
                        slot.sequence.store(
                            position.wrapping_add(LOG_CAPACITY),
                            Ordering::Release,
                        );
                        return Some(entry);
                    }
                    Err(current) => position = current,
                }
            } else if difference < 0 {
                return None;
            } else {
                position = self.dequeue.load(Ordering::Relaxed);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.enqueue.load(Ordering::Acquire) == self.dequeue.load(Ordering::Acquire)
    }
}

impl Default for ChronoLogEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{ChronoLogEngine, LogEntry, LogError, LOG_CAPACITY};

    fn entry(timestamp: u64) -> LogEntry {
        LogEntry {
            timestamp,
            context_id: 7,
            status_snapshot: [timestamp as u8; 32],
        }
    }

    #[test]
    fn ring_wraps_and_preserves_entry_fields() {
        let log = ChronoLogEngine::new();
        for timestamp in 0..LOG_CAPACITY as u64 {
            assert_eq!(log.push(entry(timestamp)), Ok(()));
        }
        assert_eq!(log.push(entry(99)), Err(LogError::Full));
        for timestamp in 0..LOG_CAPACITY as u64 {
            assert_eq!(log.pop(), Some(entry(timestamp)));
        }
        assert!(log.is_empty());

        assert_eq!(log.push(entry(100)), Ok(()));
        assert_eq!(log.pop(), Some(entry(100)));
    }
}