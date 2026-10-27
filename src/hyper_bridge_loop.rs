//! Non-blocking terminal/Ollama integration driver.

use crate::hyper_ollama_client::{
    HyperOllamaClient, IoError, NonBlockingSocket, RequestEncoder, RequestError, SendProgress,
};
use crate::sanitized_stream_reader::{
    SanitizedStreamReader, STATUS_MALFORMED, STATUS_OVERFLOW,
};
use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

const HTTP_HEADER_BYTES: usize = 512;
const SOCKET_READ_BYTES: usize = 128;
const FORWARD_BYTES: usize = 128;

#[derive(Clone, Copy, Eq, PartialEq)]
enum ChunkState {
    Size,
    SizeLf,
    Extension,
    Data,
    DataCr,
    DataLf,
    Trailer,
    TrailerLf,
    Done,
}

struct ChunkDecoder {
    state: ChunkState,
    remaining: usize,
    size: usize,
    digits: usize,
    trailer_line_nonempty: bool,
}

impl ChunkDecoder {
    const fn new() -> Self {
        Self {
            state: ChunkState::Size,
            remaining: 0,
            size: 0,
            digits: 0,
            trailer_line_nonempty: false,
        }
    }

    fn feed(
        &mut self,
        bytes: &[u8],
        reader: &mut SanitizedStreamReader<'_>,
    ) -> Result<(), BridgeErrorLoop> {
        let mut offset = 0;
        while offset < bytes.len() {
            match self.state {
                ChunkState::Size => match bytes[offset] {
                    b'0'..=b'9' => self.add_hex((bytes[offset] - b'0') as usize)?,
                    b'a'..=b'f' => self.add_hex((bytes[offset] - b'a' + 10) as usize)?,
                    b'A'..=b'F' => self.add_hex((bytes[offset] - b'A' + 10) as usize)?,
                    b';' if self.digits != 0 => self.state = ChunkState::Extension,
                    b'\r' if self.digits != 0 => self.state = ChunkState::SizeLf,
                    _ => return Err(BridgeErrorLoop::InvalidHttpResponse),
                },
                ChunkState::SizeLf => {
                    if bytes[offset] != b'\n' {
                        return Err(BridgeErrorLoop::InvalidHttpResponse);
                    }
                    self.remaining = self.size;
                    self.size = 0;
                    self.digits = 0;
                    self.state = if self.remaining == 0 {
                        ChunkState::Trailer
                    } else {
                        ChunkState::Data
                    };
                }
                ChunkState::Extension => {
                    if bytes[offset] == b'\r' {
                        self.state = ChunkState::SizeLf;
                    }
                }
                ChunkState::Data => {
                    let count = self.remaining.min(bytes.len() - offset);
                    reader.push(&bytes[offset..offset + count]);
                    offset += count;
                    self.remaining -= count;
                    if self.remaining == 0 {
                        self.state = ChunkState::DataCr;
                    }
                    continue;
                }
                ChunkState::DataCr => {
                    if bytes[offset] != b'\r' {
                        return Err(BridgeErrorLoop::InvalidHttpResponse);
                    }
                    self.state = ChunkState::DataLf;
                }
                ChunkState::DataLf => {
                    if bytes[offset] != b'\n' {
                        return Err(BridgeErrorLoop::InvalidHttpResponse);
                    }
                    self.state = ChunkState::Size;
                }
                ChunkState::Trailer => match bytes[offset] {
                    b'\r' => self.state = ChunkState::TrailerLf,
                    _ => self.trailer_line_nonempty = true,
                },
                ChunkState::TrailerLf => {
                    if bytes[offset] != b'\n' {
                        return Err(BridgeErrorLoop::InvalidHttpResponse);
                    }
                    if self.trailer_line_nonempty {
                        self.trailer_line_nonempty = false;
                        self.state = ChunkState::Trailer;
                    } else {
                        self.state = ChunkState::Done;
                    }
                }
                ChunkState::Done => return Err(BridgeErrorLoop::InvalidHttpResponse),
            }
            offset += 1;
        }
        Ok(())
    }

    fn add_hex(&mut self, digit: usize) -> Result<(), BridgeErrorLoop> {
        self.size = self
            .size
            .checked_mul(16)
            .and_then(|size| size.checked_add(digit))
            .ok_or(BridgeErrorLoop::InvalidHttpResponse)?;
        self.digits += 1;
        Ok(())
    }

    const fn is_done(&self) -> bool {
        matches!(self.state, ChunkState::Done)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeErrorLoop {
    InvalidTerminalInput,
    Request(RequestError),
    Io(IoError),
    InvalidHttpResponse,
    HttpHeaderTooLarge,
    OutputFailure,
}

impl From<RequestError> for BridgeErrorLoop {
    fn from(error: RequestError) -> Self {
        Self::Request(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoopProgress {
    Pending,
    Complete,
    StreamWarning(u8),
}

pub trait NonBlockingOutput {
    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, IoError>;
}

/// One HTTP request at a time; call `poll` from the system's regular event loop.
pub struct HyperBridgeLoop<'a> {
    request: Option<RequestEncoder<'a>>,
    headers: [u8; HTTP_HEADER_BYTES],
    headers_len: usize,
    http_body_started: bool,
    reported_flags: u8,
    read_buffer: [u8; SOCKET_READ_BYTES],
    forward_buffer: [u8; FORWARD_BYTES],
    forward_len: usize,
    forward_offset: usize,
    chunk_decoder: ChunkDecoder,
    eof: bool,
    complete: bool,
}

impl<'a> HyperBridgeLoop<'a> {
    pub const fn new() -> Self {
        Self {
            request: None,
            headers: [0; HTTP_HEADER_BYTES],
            headers_len: 0,
            http_body_started: false,
            reported_flags: 0,
            read_buffer: [0; SOCKET_READ_BYTES],
            forward_buffer: [0; FORWARD_BYTES],
            forward_len: 0,
            forward_offset: 0,
            chunk_decoder: ChunkDecoder::new(),
            eof: false,
            complete: false,
        }
    }

    /// Validate through HyperSpaceTerminal, then prepare Ollama POST.
    pub fn begin(
        &mut self,
        terminal: &HyperSpaceTerminal,
        prompt: &'a [u8],
        image: &'a [u8],
    ) -> Result<(), BridgeErrorLoop> {
        let prompt_text = core::str::from_utf8(prompt).map_err(|_| BridgeErrorLoop::InvalidTerminalInput)?;
        let mut vector = [0u8; VECTOR_BYTES];
        if !terminal.process_text_command(prompt_text, &mut vector) {
            return Err(BridgeErrorLoop::InvalidTerminalInput);
        }

        let request = HyperOllamaClient::send_vision_request(prompt, image)?;
        self.request = Some(request);
        self.headers_len = 0;
        self.http_body_started = false;
        self.reported_flags = 0;
        self.forward_len = 0;
        self.forward_offset = 0;
        self.chunk_decoder = ChunkDecoder::new();
        self.eof = false;
        self.complete = false;
        Ok(())
    }

    /// Make at most one non-blocking output or socket operation per call.
    pub fn poll<S, O>(
        &mut self,
        socket: &mut S,
        output: &mut O,
        reader: &mut SanitizedStreamReader<'_>,
    ) -> Result<LoopProgress, BridgeErrorLoop>
    where
        S: NonBlockingSocket,
        O: NonBlockingOutput,
    {
        if self.complete {
            return Ok(LoopProgress::Complete);
        }

        if let Some(request) = self.request.as_mut() {
            let progress = match request.poll_send(socket) {
                Ok(progress) => progress,
                Err(IoError::WouldBlock) => return Ok(LoopProgress::Pending),
                Err(error) => return Err(BridgeErrorLoop::Io(error)),
            };
            match progress {
                SendProgress::Pending => return Ok(LoopProgress::Pending),
                SendProgress::Complete => {
                    self.request = None;
                    return Ok(LoopProgress::Pending);
                }
            }
        }

        if self.forward_offset < self.forward_len {
            let pending = &self.forward_buffer[self.forward_offset..self.forward_len];
            let written = match output.try_write(pending) {
                Ok(written) => written,
                Err(IoError::WouldBlock) => return Ok(LoopProgress::Pending),
                Err(error) => return Err(BridgeErrorLoop::Io(error)),
            };
            if written == 0 {
                return Err(BridgeErrorLoop::OutputFailure);
            }
            self.forward_offset += written.min(pending.len());
            return Ok(LoopProgress::Pending);
        }

        if reader.output_len() != 0 {
            self.forward_len = reader.take_output(&mut self.forward_buffer);
            self.forward_offset = 0;
            return Ok(LoopProgress::Pending);
        }

        if reader.is_done() {
            self.complete = true;
            return Ok(LoopProgress::Complete);
        }
        if self.chunk_decoder.is_done() {
            self.complete = true;
            return Ok(LoopProgress::Complete);
        }
        if self.eof {
            self.complete = true;
            return Ok(LoopProgress::Complete);
        }

        let read = match socket.try_read(&mut self.read_buffer) {
            Ok(0) | Err(IoError::Closed) => {
                reader.finish();
                self.eof = true;
                let flags = reader.status_flags();
                let warning = flags & (STATUS_MALFORMED | STATUS_OVERFLOW) & !self.reported_flags;
                self.reported_flags |= warning;
                if warning != 0 {
                    return Ok(LoopProgress::StreamWarning(warning));
                }
                return Ok(LoopProgress::Complete);
            }
            Ok(count) => count.min(self.read_buffer.len()),
            Err(IoError::WouldBlock) => return Ok(LoopProgress::Pending),
            Err(error) => return Err(BridgeErrorLoop::Io(error)),
        };

        let mut body_start = 0;
        if !self.http_body_started {
            while body_start < read {
                if self.headers_len == HTTP_HEADER_BYTES {
                    return Err(BridgeErrorLoop::HttpHeaderTooLarge);
                }
                self.headers[self.headers_len] = self.read_buffer[body_start];
                self.headers_len += 1;
                body_start += 1;
                if self.headers_len >= 4
                    && &self.headers[self.headers_len - 4..self.headers_len] == b"\r\n\r\n"
                {
                    if !valid_http_status(&self.headers[..self.headers_len]) {
                        return Err(BridgeErrorLoop::InvalidHttpResponse);
                    }
                    self.http_body_started = true;
                    break;
                }
            }
        }
        if self.http_body_started && body_start < read {
            let body = &self.read_buffer[body_start..read];
            if header_is_chunked(&self.headers[..self.headers_len]) {
                self.chunk_decoder.feed(body, reader)?;
            } else {
                reader.push(body);
            }
        }

        let flags = reader.status_flags();
        let warning = flags & (STATUS_MALFORMED | STATUS_OVERFLOW) & !self.reported_flags;
        self.reported_flags |= warning;
        if warning != 0 {
            Ok(LoopProgress::StreamWarning(warning))
        } else {
            Ok(LoopProgress::Pending)
        }
    }
}

impl<'a> Default for HyperBridgeLoop<'a> {
    fn default() -> Self {
        Self::new()
    }
}

fn valid_http_status(headers: &[u8]) -> bool {
    headers.starts_with(b"HTTP/1.1 200 ") || headers.starts_with(b"HTTP/1.0 200 ")
}

fn header_is_chunked(headers: &[u8]) -> bool {
    headers
        .windows(b"transfer-encoding: chunked".len())
        .any(|window| window.eq_ignore_ascii_case(b"transfer-encoding: chunked"))
}

#[cfg(test)]
mod tests {
    use super::{
        header_is_chunked, valid_http_status, ChunkDecoder, HyperBridgeLoop, LoopProgress,
        NonBlockingOutput,
    };
    use crate::hyper_ollama_client::{IoError, NonBlockingSocket};
    use crate::sanitized_stream_reader::SanitizedStreamReader;
    use crate::terminal::HyperSpaceTerminal;
    use std::vec::Vec;

    struct MockSocket {
        request: Vec<u8>,
        response: &'static [u8],
        response_offset: usize,
    }

    impl NonBlockingSocket for MockSocket {
        fn try_write(&mut self, bytes: &[u8]) -> Result<usize, IoError> {
            self.request.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn try_read(&mut self, output: &mut [u8]) -> Result<usize, IoError> {
            if self.response_offset == self.response.len() {
                return Err(IoError::WouldBlock);
            }
            let count = output.len().min(self.response.len() - self.response_offset);
            output[..count].copy_from_slice(
                &self.response[self.response_offset..self.response_offset + count],
            );
            self.response_offset += count;
            Ok(count)
        }
    }

    #[derive(Default)]
    struct MockOutput(Vec<u8>);

    impl NonBlockingOutput for MockOutput {
        fn try_write(&mut self, bytes: &[u8]) -> Result<usize, IoError> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
    }

    #[test]
    fn accepts_only_success_status_line() {
        assert!(valid_http_status(b"HTTP/1.1 200 OK\r\n\r\n"));
        assert!(!valid_http_status(b"HTTP/1.1 500 Error\r\n\r\n"));
    }

    #[test]
    fn loop_starts_in_idle_pending_state() {
        let loop_state = HyperBridgeLoop::new();
        assert!(!loop_state.complete);
        assert_eq!(loop_state.headers_len, 0);
    }

    #[test]
    fn chunk_framing_is_removed_before_ndjson_parsing() {
        let mut output = [0; 16];
        let mut reader = SanitizedStreamReader::new(&mut output);
        let mut decoder = ChunkDecoder::new();
        decoder
            .feed(b"11\r\n{\"response\":\"x\"}\n\r\n0\r\n\r\n", &mut reader)
            .unwrap();
        assert!(decoder.is_done());
        assert_eq!(reader.output(), b"x");
    }

    #[test]
    fn chunked_header_detection_ignores_ascii_case() {
        assert!(header_is_chunked(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n"));
        assert!(header_is_chunked(b"transfer-encoding: CHUNKED\r\n"));
    }

    #[test]
    fn integration_loop_routes_and_streams_chunked_ollama_response() {
        let mut bridge_loop = HyperBridgeLoop::new();
        let terminal = HyperSpaceTerminal::new();
        bridge_loop
            .begin(&terminal, b"describe", &[1, 2, 3])
            .unwrap();

        let mut socket = MockSocket {
            request: Vec::new(),
            response: b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n11\r\n{\"response\":\"x\"}\n\r\n0\r\n\r\n",
            response_offset: 0,
        };
        let mut output = MockOutput::default();
        let mut response_buffer = [0; 32];
        let mut reader = SanitizedStreamReader::new(&mut response_buffer);

        let mut completed = false;
        for _ in 0..100 {
            if bridge_loop.poll(&mut socket, &mut output, &mut reader).unwrap()
                == LoopProgress::Complete
            {
                completed = true;
                break;
            }
        }
        assert!(completed);
        assert!(socket.request.starts_with(b"POST /api/generate HTTP/1.1\r\n"));
        assert!(output.0.ends_with(b"x"));
    }
}