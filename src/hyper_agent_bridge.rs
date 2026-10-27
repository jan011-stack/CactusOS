//! Four isolated, allocation-free SPSC channels with atomic ready flags.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

pub const AGENT_COUNT: usize = 4;
pub const AGENT_QUEUE_CAPACITY: usize = 8;
pub const AGENT_PAYLOAD_BYTES: usize = 256;
const QUEUE_MASK: usize = AGENT_QUEUE_CAPACITY - 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AgentIdentity {
    Alex = 0x01,
    Astyria = 0x02,
    Aurelius = 0x03,
    Si1 = 0x04,
}

impl AgentIdentity {
    const fn index(self) -> usize {
        self as usize - 1
    }

    const fn at(index: usize) -> Self {
        match index {
            0 => Self::Alex,
            1 => Self::Astyria,
            2 => Self::Aurelius,
            _ => Self::Si1,
        }
    }

    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Alex => "Alex",
            Self::Astyria => "Astyria",
            Self::Aurelius => "Aurelius",
            Self::Si1 => "SI_1",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadKind {
    TerminalText,
    VisionToken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeError {
    PayloadTooLarge,
    QueueFull,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentMessage {
    pub agent: AgentIdentity,
    pub kind: PayloadKind,
    length: u16,
    payload: [u8; AGENT_PAYLOAD_BYTES],
}

impl AgentMessage {
    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.length as usize]
    }
}

macro_rules! uninitialized_slots {
    () => {
        [
            UnsafeCell::new(MaybeUninit::uninit()),
            UnsafeCell::new(MaybeUninit::uninit()),
            UnsafeCell::new(MaybeUninit::uninit()),
            UnsafeCell::new(MaybeUninit::uninit()),
            UnsafeCell::new(MaybeUninit::uninit()),
            UnsafeCell::new(MaybeUninit::uninit()),
            UnsafeCell::new(MaybeUninit::uninit()),
            UnsafeCell::new(MaybeUninit::uninit()),
        ]
    };
}

struct AgentQueue {
    slots: [UnsafeCell<MaybeUninit<AgentMessage>>; AGENT_QUEUE_CAPACITY],
    head: AtomicUsize,
    tail: AtomicUsize,
}

// SAFETY: split() creates exactly one non-cloneable producer and consumer per queue.
unsafe impl Sync for AgentQueue {}

impl AgentQueue {
    const fn new() -> Self {
        Self {
            slots: uninitialized_slots!(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn is_empty(&self) -> bool {
        self.tail.load(Ordering::Acquire) == self.head.load(Ordering::Acquire)
    }
}

/// Static storage owner. Call `split` once to obtain the four unique channel pairs.
pub struct HyperAgentBridge {
    queues: [AgentQueue; AGENT_COUNT],
    ready_flags: AtomicU8,
}

impl HyperAgentBridge {
    pub const fn new() -> Self {
        Self {
            queues: [
                AgentQueue::new(),
                AgentQueue::new(),
                AgentQueue::new(),
                AgentQueue::new(),
            ],
            ready_flags: AtomicU8::new(0),
        }
    }

    pub fn split(&mut self) -> (
        [AgentProducer<'_>; AGENT_COUNT],
        [AgentConsumer<'_>; AGENT_COUNT],
    ) {
        let queues = &self.queues;
        let ready_flags = &self.ready_flags;
        let producers = core::array::from_fn(|index| AgentProducer {
            queue: &queues[index],
            ready_flags,
            agent: AgentIdentity::at(index),
        });
        let consumers = core::array::from_fn(|index| AgentConsumer {
            queue: &queues[index],
            ready_flags,
            agent: AgentIdentity::at(index),
        });
        (producers, consumers)
    }

    /// Readiness is a hint bitset; per-queue head/tail remain authoritative.
    pub fn ready_mask(&self) -> u8 {
        self.ready_flags.load(Ordering::Acquire)
    }
}

impl Default for HyperAgentBridge {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AgentProducer<'a> {
    queue: &'a AgentQueue,
    ready_flags: &'a AtomicU8,
    agent: AgentIdentity,
}

impl AgentProducer<'_> {
    pub const fn identity(&self) -> AgentIdentity {
        self.agent
    }

    pub fn ready_mask(&self) -> u8 {
        self.ready_flags.load(Ordering::Acquire)
    }

    pub fn publish_terminal_text(&mut self, text: &[u8]) -> Result<(), BridgeError> {
        self.publish(PayloadKind::TerminalText, text)
    }

    pub fn publish_vision_token(
        &mut self,
        token: &[u8; 32],
    ) -> Result<(), BridgeError> {
        self.publish(PayloadKind::VisionToken, token)
    }

    fn publish(&mut self, kind: PayloadKind, bytes: &[u8]) -> Result<(), BridgeError> {
        if bytes.len() > AGENT_PAYLOAD_BYTES {
            return Err(BridgeError::PayloadTooLarge);
        }
        let head = self.queue.head.load(Ordering::Relaxed);
        let tail = self.queue.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= AGENT_QUEUE_CAPACITY {
            return Err(BridgeError::QueueFull);
        }

        let mut message = AgentMessage {
            agent: self.agent,
            kind,
            length: bytes.len() as u16,
            payload: [0; AGENT_PAYLOAD_BYTES],
        };
        message.payload[..bytes.len()].copy_from_slice(bytes);
        let slot = &self.queue.slots[head & QUEUE_MASK];
        // SAFETY: the unique producer owns this slot until publishing the new head.
        unsafe { (*slot.get()).write(message) };
        self.queue.head.store(head.wrapping_add(1), Ordering::Release);
        self.ready_flags
            .fetch_or(1 << self.agent.index(), Ordering::Release);
        Ok(())
    }
}

pub struct AgentConsumer<'a> {
    queue: &'a AgentQueue,
    ready_flags: &'a AtomicU8,
    agent: AgentIdentity,
}

impl AgentConsumer<'_> {
    pub const fn identity(&self) -> AgentIdentity {
        self.agent
    }

    pub fn ready_mask(&self) -> u8 {
        self.ready_flags.load(Ordering::Acquire)
    }

    pub fn try_receive(&mut self) -> Option<AgentMessage> {
        let tail = self.queue.tail.load(Ordering::Relaxed);
        let head = self.queue.head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let slot = &self.queue.slots[tail & QUEUE_MASK];
        // SAFETY: the unique consumer owns this initialized slot until advancing tail.
        let message = unsafe { (*slot.get()).assume_init_read() };
        self.queue.tail.store(tail.wrapping_add(1), Ordering::Release);

        let mask = !(1 << self.agent.index());
        self.ready_flags.fetch_and(mask, Ordering::AcqRel);
        if !self.queue.is_empty() {
            self.ready_flags
                .fetch_or(1 << self.agent.index(), Ordering::Release);
        }
        Some(message)
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentIdentity, BridgeError, HyperAgentBridge, PayloadKind};

    #[test]
    fn channels_are_isolated_and_ready_bits_track_pending_data() {
        let mut bridge = HyperAgentBridge::new();
        let (mut producers, mut consumers) = bridge.split();
        producers[0].publish_terminal_text(b"hello").unwrap();
        producers[2].publish_vision_token(&[7; 32]).unwrap();

        assert_eq!(consumers[0].ready_mask(), 0b0101);
        let text = consumers[0].try_receive().unwrap();
        assert_eq!(text.agent, AgentIdentity::Alex);
        assert_eq!(text.kind, PayloadKind::TerminalText);
        assert_eq!(text.payload(), b"hello");
        assert!(consumers[1].try_receive().is_none());
        let token = consumers[2].try_receive().unwrap();
        assert_eq!(token.kind, PayloadKind::VisionToken);
        assert_eq!(token.payload(), &[7; 32]);
        assert_eq!(consumers[0].ready_mask(), 0);
    }

    #[test]
    fn queue_capacity_and_payload_bounds_are_enforced() {
        let mut bridge = HyperAgentBridge::new();
        let (mut producers, _) = bridge.split();
        assert_eq!(
            producers[0].publish_terminal_text(&[0; 257]),
            Err(BridgeError::PayloadTooLarge)
        );
        for _ in 0..8 {
            producers[0].publish_terminal_text(b"x").unwrap();
        }
        assert_eq!(producers[0].publish_terminal_text(b"x"), Err(BridgeError::QueueFull));
    }
}