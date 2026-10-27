//! Bounded token draining, sanitization, and rendering into caller-owned display memory.

use crate::chrono_log::{ChronoLogEngine, LogEntry, LogError};
use crate::sanitized_stream_reader::SanitizedStreamReader;
use crate::hyper_token_filter::{FilterError, HyperTokenFilter};
use crate::terminal::VECTOR_BYTES;

pub const STREAM_CHUNK_BYTES: usize = 128;
const LOG_BOUNDARY_FAILURE: u8 = 0xe1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamerError {
    InvalidDisplayGeometry { logged: bool },
    Filter { error: FilterError, logged: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamerProgress {
    pub consumed: usize,
    pub rendered: usize,
    pub boundary_failure_logged: bool,
}

/// Real-time stream renderer. `display` is a mapped, power-of-two byte matrix.
pub struct HyperVisionStreamer {
    filter: HyperTokenFilter,
    chunk: [u8; STREAM_CHUNK_BYTES],
    display_cursor: usize,
    total_rendered: u64,
}

impl HyperVisionStreamer {
    pub const fn new() -> Self {
        Self {
            filter: HyperTokenFilter::new(),
            chunk: [0; STREAM_CHUNK_BYTES],
            display_cursor: 0,
            total_rendered: 0,
        }
    }

    pub const fn display_cursor(&self) -> usize {
        self.display_cursor
    }

    /// Drain at most one fixed chunk; use the scheduler tick as the PI=4 phase.
    pub fn pump(
        &mut self,
        reader: &mut SanitizedStreamReader<'_>,
        display: &mut [u8],
        log: &ChronoLogEngine,
        timestamp: u64,
        context_id: u32,
        scheduler_tick: u64,
    ) -> Result<StreamerProgress, StreamerError> {
        if display.is_empty() || !display.len().is_power_of_two() {
            let logged = self.record_failure(log, timestamp, context_id);
            reader.purge_buffers();
            return Err(StreamerError::InvalidDisplayGeometry { logged });
        }

        let consumed = reader.take_output(&mut self.chunk);
        let mut length = consumed;
        if let Err(error) = self.filter.sanitize_in_place(&mut self.chunk, &mut length) {
            reader.purge_buffers();
            let logged = self.record_failure(log, timestamp, context_id);
            return Err(StreamerError::Filter { error, logged });
        }

        let mask = display.len() - 1;
        for index in 0..length {
            let grid_phase = ((scheduler_tick.wrapping_add(self.total_rendered) & 3) as u32) << 1;
            let byte = self.chunk[index];
            let rendered = if grid_phase == 0 {
                byte
            } else {
                (byte << grid_phase) | (byte >> (8 - grid_phase))
            };
            display[self.display_cursor] = rendered;
            self.display_cursor = (self.display_cursor + 1) & mask;
            self.total_rendered = self.total_rendered.wrapping_add(1);
        }

        self.chunk[..length].fill(0);
        Ok(StreamerProgress {
            consumed,
            rendered: length,
            boundary_failure_logged: false,
        })
    }

    pub fn finish(
        &mut self,
        reader: &mut SanitizedStreamReader<'_>,
        log: &ChronoLogEngine,
        timestamp: u64,
        context_id: u32,
    ) -> Result<(), StreamerError> {
        if !self.filter.finish() {
            reader.purge_buffers();
            self.record_failure(log, timestamp, context_id);
            let logged = self.record_failure(log, timestamp, context_id);
            return Err(StreamerError::Filter {
                error: FilterError::InvalidUtf8,
                logged,
            });
        }
        Ok(())
    }

    fn record_failure(&self, log: &ChronoLogEngine, timestamp: u64, context_id: u32) -> bool {
        let mut status_snapshot = [0; VECTOR_BYTES];
        status_snapshot[0] = LOG_BOUNDARY_FAILURE;
        match log.push(LogEntry {
            timestamp,
            context_id,
            status_snapshot,
        }) {
            Ok(()) => true,
            Err(LogError::Full) => false,
        }
    }
}

impl Default for HyperVisionStreamer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{HyperVisionStreamer, StreamerError};
    use crate::chrono_log::ChronoLogEngine;
    use crate::hyper_token_filter::FilterError;
    use crate::sanitized_stream_reader::SanitizedStreamReader;

    #[test]
    fn pumps_sanitized_tokens_into_wrapping_display() {
        let mut response = [0; 32];
        let mut reader = SanitizedStreamReader::new(&mut response);
        reader.push(b"{\"response\":\"abcd\"}\n");
        let mut display = [0; 4];
        let mut streamer = HyperVisionStreamer::new();
        let log = ChronoLogEngine::new();
        let result = streamer.pump(&mut reader, &mut display, &log, 10, 2, 0).unwrap();
        assert_eq!(result.rendered, 4);
        let expected = [b'a', b'b'.rotate_left(2), b'c'.rotate_left(4), b'd'.rotate_left(6)];
        assert_eq!(display, expected);
        assert_eq!(streamer.display_cursor(), 0);
    }

    #[test]
    fn blocked_marker_purges_buffers_and_logs_boundary_failure() {
        let mut response = [0; 64];
        let mut reader = SanitizedStreamReader::new(&mut response);
        reader.push(b"{\"response\":\"<|im_start|>\"}\n");
        let mut display = [0; 16];
        let mut streamer = HyperVisionStreamer::new();
        let log = ChronoLogEngine::new();
        assert_eq!(
            streamer.pump(&mut reader, &mut display, &log, 11, 4, 0),
            Err(StreamerError::Filter {
                error: FilterError::ModelControlMarker,
                logged: true,
            })
        );
        assert!(response.iter().all(|byte| *byte == 0));
        assert_eq!(log.pop().unwrap().status_snapshot[0], 0xe1);
        assert_eq!(display, [0; 16]);
    }
}