//! Fixed-size immutable chat records with plain-text rendering.

use crate::hyper_agent_bridge::AgentIdentity;

pub const CHAT_CAPACITY: usize = 16;
pub const CHAT_PAYLOAD_BYTES: usize = 512;
const CHAT_MASK: usize = CHAT_CAPACITY - 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatError {
    PayloadTooLong,
    OutputTooSmall,
}

#[derive(Clone, Copy)]
pub struct ChatEntry {
    pub timestamp: u64,
    pub agent: AgentIdentity,
    length: u16,
    payload: [u8; CHAT_PAYLOAD_BYTES],
}

impl ChatEntry {
    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.length as usize]
    }
}

/// Bounded history: records are immutable after append and retained until ring wrap.
pub struct HyperChatStreamEngine {
    entries: [ChatEntry; CHAT_CAPACITY],
    next_sequence: u64,
    len: usize,
}

impl HyperChatStreamEngine {
    pub const fn new() -> Self {
        Self {
            entries: [const {
                ChatEntry {
                    timestamp: 0,
                    agent: AgentIdentity::Alex,
                    length: 0,
                    payload: [0; CHAT_PAYLOAD_BYTES],
                }
            }; CHAT_CAPACITY],
            next_sequence: 0,
            len: 0,
        }
    }

    pub fn append(
        &mut self,
        timestamp: u64,
        agent: AgentIdentity,
        payload: &[u8],
    ) -> Result<u64, ChatError> {
        if payload.len() > CHAT_PAYLOAD_BYTES {
            return Err(ChatError::PayloadTooLong);
        }
        let sequence = self.next_sequence;
        let index = sequence as usize & CHAT_MASK;
        let mut entry = ChatEntry {
            timestamp,
            agent,
            length: payload.len() as u16,
            payload: [0; CHAT_PAYLOAD_BYTES],
        };
        entry.payload[..payload.len()].copy_from_slice(payload);
        self.entries[index] = entry;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.len = (self.len + 1).min(CHAT_CAPACITY);
        Ok(sequence)
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, sequence: u64) -> Option<&ChatEntry> {
        let oldest = self.next_sequence.saturating_sub(self.len as u64);
        if sequence < oldest || sequence >= self.next_sequence {
            return None;
        }
        let entry = &self.entries[sequence as usize & CHAT_MASK];
        Some(entry)
    }

    /// Render `timestamp|Agent|payload\n` into caller-owned plain-text storage.
    pub fn format_entry(&self, sequence: u64, output: &mut [u8]) -> Result<usize, ChatError> {
        let entry = self.get(sequence).ok_or(ChatError::OutputTooSmall)?;
        let mut writer = TextWriter::new(output);
        writer.write_u64(entry.timestamp)?;
        writer.write_byte(b'|')?;
        writer.write_bytes(entry.agent.prefix().as_bytes())?;
        writer.write_byte(b'|')?;
        writer.write_bytes(entry.payload())?;
        writer.write_byte(b'\n')?;
        Ok(writer.position)
    }
}

impl Default for HyperChatStreamEngine {
    fn default() -> Self {
        Self::new()
    }
}

struct TextWriter<'a> {
    output: &'a mut [u8],
    position: usize,
}

impl<'a> TextWriter<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, position: 0 }
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), ChatError> {
        let slot = self.output.get_mut(self.position).ok_or(ChatError::OutputTooSmall)?;
        *slot = byte;
        self.position += 1;
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), ChatError> {
        let end = self.position.checked_add(bytes.len()).ok_or(ChatError::OutputTooSmall)?;
        let target = self.output.get_mut(self.position..end).ok_or(ChatError::OutputTooSmall)?;
        target.copy_from_slice(bytes);
        self.position = end;
        Ok(())
    }

    fn write_u64(&mut self, mut value: u64) -> Result<(), ChatError> {
        let mut digits = [0u8; 20];
        let mut length = 0;
        loop {
            digits[length] = b'0' + (value % 10) as u8;
            length += 1;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        while length > 0 {
            length -= 1;
            self.write_byte(digits[length])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatError, HyperChatStreamEngine, CHAT_CAPACITY};
    use crate::hyper_agent_bridge::AgentIdentity;

    #[test]
    fn records_wrap_and_render_as_plain_text() {
        let mut stream = HyperChatStreamEngine::new();
        for index in 0..CHAT_CAPACITY as u64 + 1 {
            stream.append(1000 + index, AgentIdentity::Si1, b"hello").unwrap();
        }
        assert!(stream.get(0).is_none());
        assert_eq!(stream.get(1).unwrap().timestamp, 1001);

        let mut output = [0; 64];
        let written = stream.format_entry(CHAT_CAPACITY as u64, &mut output).unwrap();
        assert_eq!(&output[..written], b"1016|SI_1|hello\n");
    }

    #[test]
    fn rejects_oversized_payload_and_short_render_buffer() {
        let mut stream = HyperChatStreamEngine::new();
        assert_eq!(
            stream.append(1, AgentIdentity::Alex, &[0; 513]),
            Err(ChatError::PayloadTooLong)
        );
        stream.append(1, AgentIdentity::Alex, b"x").unwrap();
        assert_eq!(stream.format_entry(0, &mut [0; 1]), Err(ChatError::OutputTooSmall));
    }
}