//! Non-blocking, bounded receiver for integrity-checked data update frames.

use crate::chrono_log::LogEntry;
use crate::hyper_chrono_guard::{GuardError, HyperChronoGuard, UpdateKind};
use crate::hyper_ollama_client::{IoError, NonBlockingSocket};
use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

pub const UPDATE_BLOCK_MAX: usize = 256;
const HEADER_BYTES: usize = 39;
const READ_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateError {
    Io(IoError),
    MalformedFrame,
    Guard(GuardError),
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateProgress {
    Pending,
    Staged {
        kind: UpdateKind,
        length: usize,
        feature_delta: [u8; VECTOR_BYTES],
    },
}

/// The platform adapter must establish peer authenticity before exposing bytes.
pub trait AuthenticatedUpdateSource: NonBlockingSocket {
    fn source_id(&self) -> u32;
    fn is_authenticated(&self) -> bool;
}

struct FrameDecoder {
    header: [u8; HEADER_BYTES],
    header_len: usize,
    payload: [u8; UPDATE_BLOCK_MAX],
    payload_len: usize,
    payload_read: usize,
    expected_hash: [u8; VECTOR_BYTES],
    kind: UpdateKind,
    ready: bool,
}

impl FrameDecoder {
    const fn new() -> Self {
        Self {
            header: [0; HEADER_BYTES],
            header_len: 0,
            payload: [0; UPDATE_BLOCK_MAX],
            payload_len: 0,
            payload_read: 0,
            expected_hash: [0; VECTOR_BYTES],
            kind: UpdateKind::Data,
            ready: false,
        }
    }

    fn feed(&mut self, bytes: &[u8]) -> Result<usize, UpdateError> {
        let mut offset = 0;
        while offset < bytes.len() && !self.ready {
            if self.header_len < HEADER_BYTES {
                let count = (HEADER_BYTES - self.header_len).min(bytes.len() - offset);
                self.header[self.header_len..self.header_len + count]
                    .copy_from_slice(&bytes[offset..offset + count]);
                self.header_len += count;
                offset += count;
                if self.header_len == HEADER_BYTES {
                    self.parse_header()?;
                    if self.payload_len == 0 {
                        self.ready = true;
                    }
                }
                continue;
            }

            let count = (self.payload_len - self.payload_read).min(bytes.len() - offset);
            self.payload[self.payload_read..self.payload_read + count]
                .copy_from_slice(&bytes[offset..offset + count]);
            self.payload_read += count;
            offset += count;
            self.ready = self.payload_read == self.payload_len;
        }
        Ok(offset)
    }

    fn parse_header(&mut self) -> Result<(), UpdateError> {
        if &self.header[..4] != b"HUPD" {
            return Err(UpdateError::MalformedFrame);
        }
        self.kind = match self.header[4] {
            0 => UpdateKind::Data,
            1 => UpdateKind::ProtectedData,
            _ => return Err(UpdateError::MalformedFrame),
        };
        self.payload_len = u16::from_le_bytes([self.header[5], self.header[6]]) as usize;
        if self.payload_len == 0 || self.payload_len > UPDATE_BLOCK_MAX {
            return Err(UpdateError::MalformedFrame);
        }
        self.expected_hash.copy_from_slice(&self.header[7..HEADER_BYTES]);
        Ok(())
    }

    fn reset(&mut self) {
        self.header_len = 0;
        self.payload_len = 0;
        self.payload_read = 0;
        self.expected_hash = [0; VECTOR_BYTES];
        self.kind = UpdateKind::Data;
        self.ready = false;
    }
}

/// Call `poll` from a scheduler callback; one call performs at most one socket read.
/// The socket adapter should be connected to an authenticated, trusted update source.
pub struct HyperUpdateEngine {
    decoder: FrameDecoder,
    input: [u8; READ_BYTES],
    input_len: usize,
    input_offset: usize,
}

impl HyperUpdateEngine {
    pub const fn new() -> Self {
        Self {
            decoder: FrameDecoder::new(),
            input: [0; READ_BYTES],
            input_len: 0,
            input_offset: 0,
        }
    }

    /// Process at most one frame per call; local snapshot should come from ChronoLog.
    pub fn poll<S: AuthenticatedUpdateSource>(
        &mut self,
        source: &mut S,
        terminal: &HyperSpaceTerminal,
        guard: &mut HyperChronoGuard,
        root_token: Option<&[u8; VECTOR_BYTES]>,
        local_log_entry: &LogEntry,
        shadow: &mut [u8],
    ) -> Result<UpdateProgress, UpdateError> {
        let source_id = source.source_id();
        if guard.is_blacklisted(source_id) {
            return Err(UpdateError::Guard(GuardError::SourceBlacklisted));
        }
        if !source.is_authenticated() {
            guard.reject_source(source_id, shadow);
            return Err(UpdateError::MalformedFrame);
        }

        if self.decoder.ready {
            let delta = guard
                .verify_and_stage(
                    terminal,
                    source_id,
                    self.decoder.kind,
                    &self.decoder.expected_hash,
                    &self.decoder.payload[..self.decoder.payload_len],
                    root_token,
                    &local_log_entry.status_snapshot,
                    shadow,
                )
                .map_err(UpdateError::Guard)?;
            let kind = self.decoder.kind;
            let length = self.decoder.payload_len;
            self.decoder.reset();
            return Ok(UpdateProgress::Staged {
                kind,
                length,
                feature_delta: delta,
            });
        }

        if self.input_offset == self.input_len {
            self.input_len = match source.try_read(&mut self.input) {
                Ok(0) | Err(IoError::Closed) => return Err(UpdateError::Closed),
                Ok(count) => count.min(self.input.len()),
                Err(IoError::WouldBlock) => return Ok(UpdateProgress::Pending),
                Err(error) => return Err(UpdateError::Io(error)),
            };
            self.input_offset = 0;
        }

        let consumed = match self.decoder.feed(&self.input[self.input_offset..self.input_len]) {
            Ok(consumed) => consumed,
            Err(error) => {
                guard.reject_source(source_id, shadow);
                self.input_len = 0;
                self.input_offset = 0;
                self.decoder.reset();
                return Err(error);
            }
        };
        self.input_offset += consumed;
        Ok(UpdateProgress::Pending)
    }
}

impl Default for HyperUpdateEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthenticatedUpdateSource, HyperUpdateEngine, UpdateProgress};
    use crate::chrono_log::LogEntry;
    use crate::hyper_chrono_guard::HyperChronoGuard;
    use crate::hyper_ollama_client::{IoError, NonBlockingSocket};
    use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};
    use sha2::{Digest, Sha256};

    struct Source {
        bytes: [u8; 64],
        length: usize,
        offset: usize,
    }

    impl NonBlockingSocket for Source {
        fn try_write(&mut self, _: &[u8]) -> Result<usize, IoError> {
            Err(IoError::Failure)
        }

        fn try_read(&mut self, output: &mut [u8]) -> Result<usize, IoError> {
            if self.offset == self.length {
                return Err(IoError::WouldBlock);
            }
            let count = output.len().min(self.length - self.offset);
            output[..count].copy_from_slice(&self.bytes[self.offset..self.offset + count]);
            self.offset += count;
            Ok(count)
        }
    }

    impl AuthenticatedUpdateSource for Source {
        fn source_id(&self) -> u32 { 9 }
        fn is_authenticated(&self) -> bool { true }
    }

    #[test]
    fn parses_hash_verified_data_frame_into_shadow_buffer() {
        let payload = b"delta";
        let hash: [u8; VECTOR_BYTES] = Sha256::digest(payload).into();
        let mut bytes = [0; 64];
        bytes[..4].copy_from_slice(b"HUPD");
        bytes[4] = 0;
        bytes[5..7].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        bytes[7..39].copy_from_slice(&hash);
        bytes[39..39 + payload.len()].copy_from_slice(payload);
        let mut source = Source { bytes, length: 39 + payload.len(), offset: 0 };
        let mut update = HyperUpdateEngine::new();
        let mut guard = HyperChronoGuard::new();
        let terminal = HyperSpaceTerminal::new();
        let mut shadow = [0xaa; 16];
        let local = LogEntry {
            timestamp: 1,
            context_id: 2,
            status_snapshot: [0; VECTOR_BYTES],
        };

        assert_eq!(update.poll(&mut source, &terminal, &mut guard, None, &local, &mut shadow), Ok(UpdateProgress::Pending));
        let result = update.poll(&mut source, &terminal, &mut guard, None, &local, &mut shadow).unwrap();
        assert!(matches!(result, UpdateProgress::Staged { length: 5, .. }));
        assert_eq!(&shadow[..5], payload);
    }
}