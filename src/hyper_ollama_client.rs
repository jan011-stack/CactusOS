//! Allocation-free HTTP request encoder for the local Ollama vision endpoint.

pub const OLLAMA_HOST: &str = "127.0.0.1:11434";
pub const OLLAMA_PATH: &str = "/api/generate";
const PENDING_BYTES: usize = 256;
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoError {
    WouldBlock,
    Closed,
    Failure,
}

/// OS adapter for a non-blocking TCP socket connected to Ollama.
pub trait NonBlockingSocket {
    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, IoError>;
    fn try_read(&mut self, bytes: &mut [u8]) -> Result<usize, IoError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestError {
    InvalidPromptUtf8,
    RequestTooLarge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SendProgress {
    Pending,
    Complete,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Section {
    Header,
    Prefix,
    Prompt,
    ImageField,
    Image,
    Suffix,
    Done,
}

/// Stateless constructor namespace for the request encoder.
pub struct HyperOllamaClient;

pub fn send_vision_request<'a>(
    prompt: &'a [u8],
    image_bytes: &'a [u8],
) -> Result<RequestEncoder<'a>, RequestError> {
    HyperOllamaClient::send_vision_request(prompt, image_bytes)
}

impl HyperOllamaClient {
    /// Build a streaming JSON request; the prompt and image remain borrowed.
    pub fn send_vision_request<'a>(
        prompt: &'a [u8],
        image_bytes: &'a [u8],
    ) -> Result<RequestEncoder<'a>, RequestError> {
        if core::str::from_utf8(prompt).is_err() {
            return Err(RequestError::InvalidPromptUtf8);
        }
        let prompt_json_len = escaped_json_length(prompt)?;
        let base64_len = image_bytes
            .len()
            .checked_add(2)
            .and_then(|length| length.checked_div(3))
            .and_then(|groups| groups.checked_mul(4))
            .ok_or(RequestError::RequestTooLarge)?;
        let body_len = b"{\"model\":\"llama3.2-vision\",\"prompt\":\"".len()
            .checked_add(prompt_json_len)
            .and_then(|length| length.checked_add(b"\",\"images\":[\"".len()))
            .and_then(|length| length.checked_add(base64_len))
            .and_then(|length| length.checked_add(b"\"],\"stream\":true}".len()))
            .ok_or(RequestError::RequestTooLarge)?;

        let mut request = RequestEncoder {
            prompt,
            image: image_bytes,
            section: Section::Header,
            section_offset: 0,
            prompt_offset: 0,
            escape: [0; 6],
            escape_len: 0,
            escape_offset: 0,
            image_group_offset: 0,
            image_char_offset: 0,
            header: [0; 192],
            header_len: 0,
            pending: [0; PENDING_BYTES],
            pending_len: 0,
            pending_offset: 0,
        };
        request.header_len = make_header(&mut request.header, body_len)?;
        Ok(request)
    }
}

pub struct RequestEncoder<'a> {
    prompt: &'a [u8],
    image: &'a [u8],
    section: Section,
    section_offset: usize,
    prompt_offset: usize,
    escape: [u8; 6],
    escape_len: usize,
    escape_offset: usize,
    image_group_offset: usize,
    image_char_offset: usize,
    header: [u8; 192],
    header_len: usize,
    pending: [u8; PENDING_BYTES],
    pending_len: usize,
    pending_offset: usize,
}

impl RequestEncoder<'_> {
    /// Perform at most one socket write, preserving partial progress between polls.
    pub fn poll_send<S: NonBlockingSocket>(
        &mut self,
        socket: &mut S,
    ) -> Result<SendProgress, IoError> {
        if self.pending_offset == self.pending_len {
            self.pending_len = 0;
            self.pending_offset = 0;
            while self.pending_len < self.pending.len() {
                let Some(byte) = self.next_byte() else {
                    break;
                };
                self.pending[self.pending_len] = byte;
                self.pending_len += 1;
            }
        }
        if self.pending_len == 0 {
            return Ok(SendProgress::Complete);
        }
        let written = socket.try_write(&self.pending[self.pending_offset..self.pending_len])?;
        if written == 0 {
            return Err(IoError::Closed);
        }
        self.pending_offset += written.min(self.pending_len - self.pending_offset);
        if self.pending_offset == self.pending_len && self.section == Section::Done {
            Ok(SendProgress::Complete)
        } else {
            Ok(SendProgress::Pending)
        }
    }

    fn next_byte(&mut self) -> Option<u8> {
        loop {
            match self.section {
                Section::Header => {
                    if let Some(&byte) = self.header[..self.header_len].get(self.section_offset) {
                        self.section_offset += 1;
                        return Some(byte);
                    }
                    self.advance(Section::Prefix);
                }
                Section::Prefix => {
                    if let Some(byte) = static_byte(
                        b"{\"model\":\"llama3.2-vision\",\"prompt\":\"",
                        &mut self.section_offset,
                    ) {
                        return Some(byte);
                    }
                    self.advance(Section::Prompt);
                }
                Section::Prompt => {
                    if self.escape_offset < self.escape_len {
                        let byte = self.escape[self.escape_offset];
                        self.escape_offset += 1;
                        return Some(byte);
                    }
                    if self.prompt_offset == self.prompt.len() {
                        self.advance(Section::ImageField);
                        continue;
                    }
                    let byte = self.prompt[self.prompt_offset];
                    self.prompt_offset += 1;
                    match byte {
                        b'"' | b'\\' => {
                            self.escape[0] = b'\\';
                            self.escape[1] = byte;
                            self.escape_len = 2;
                            self.escape_offset = 1;
                            return Some(self.escape[0]);
                        }
                        0..=0x1f => {
                            const HEX: &[u8; 16] = b"0123456789abcdef";
                            self.escape = [b'\\', b'u', b'0', b'0', HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]];
                            self.escape_len = 6;
                            self.escape_offset = 1;
                            return Some(self.escape[0]);
                        }
                        _ => return Some(byte),
                    }
                }
                Section::ImageField => {
                    if let Some(byte) = static_byte(b"\",\"images\":[\"", &mut self.section_offset) {
                        return Some(byte);
                    }
                    self.advance(Section::Image);
                }
                Section::Image => {
                    if self.image_group_offset >= self.image.len() {
                        self.advance(Section::Suffix);
                        continue;
                    }
                    let remain = (self.image.len() - self.image_group_offset).min(3);
                    let a = self.image[self.image_group_offset];
                    let b = if remain > 1 { self.image[self.image_group_offset + 1] } else { 0 };
                    let c = if remain > 2 { self.image[self.image_group_offset + 2] } else { 0 };
                    let sextets = [a >> 2, ((a << 4) | (b >> 4)) & 63, ((b << 2) | (c >> 6)) & 63, c & 63];
                    let index = self.image_char_offset;
                    let output = if index >= remain + 1 {
                        b'='
                    } else {
                        BASE64[sextets[index] as usize]
                    };
                    self.image_char_offset += 1;
                    if self.image_char_offset == 4 {
                        self.image_char_offset = 0;
                        self.image_group_offset += remain;
                    }
                    return Some(output);
                }
                Section::Suffix => {
                    if let Some(byte) = static_byte(b"\"],\"stream\":true}", &mut self.section_offset) {
                        return Some(byte);
                    }
                    self.advance(Section::Done);
                }
                Section::Done => return None,
            }
        }
    }

    fn advance(&mut self, section: Section) {
        self.section = section;
        self.section_offset = 0;
    }
}

fn static_byte(bytes: &[u8], offset: &mut usize) -> Option<u8> {
    let byte = bytes.get(*offset).copied()?;
    *offset += 1;
    Some(byte)
}

fn escaped_json_length(bytes: &[u8]) -> Result<usize, RequestError> {
    bytes.iter().try_fold(0usize, |length, byte| {
        length
            .checked_add(match byte {
                b'"' | b'\\' => 2,
                0..=0x1f => 6,
                _ => 1,
            })
            .ok_or(RequestError::RequestTooLarge)
    })
}

fn make_header(output: &mut [u8; 192], content_length: usize) -> Result<usize, RequestError> {
    let mut writer = HeaderWriter { output, position: 0 };
    writer.write(b"POST /api/generate HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nContent-Type: application/json\r\nAccept: application/x-ndjson\r\nContent-Length: ")?;
    writer.write_decimal(content_length)?;
    writer.write(b"\r\nConnection: keep-alive\r\n\r\n")?;
    Ok(writer.position)
}

struct HeaderWriter<'a> {
    output: &'a mut [u8],
    position: usize,
}

impl HeaderWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), RequestError> {
        let end = self.position.checked_add(bytes.len()).ok_or(RequestError::RequestTooLarge)?;
        let target = self.output.get_mut(self.position..end).ok_or(RequestError::RequestTooLarge)?;
        target.copy_from_slice(bytes);
        self.position = end;
        Ok(())
    }

    fn write_decimal(&mut self, mut number: usize) -> Result<(), RequestError> {
        let mut digits = [0u8; 20];
        let mut length = 0;
        loop {
            digits[length] = b'0' + (number % 10) as u8;
            length += 1;
            number /= 10;
            if number == 0 { break; }
        }
        while length > 0 {
            length -= 1;
            self.write(&digits[length..length + 1])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{send_vision_request, IoError, NonBlockingSocket, SendProgress};
    use std::vec::Vec;

    struct Capture(Vec<u8>);

    impl NonBlockingSocket for Capture {
        fn try_write(&mut self, bytes: &[u8]) -> Result<usize, IoError> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn try_read(&mut self, _: &mut [u8]) -> Result<usize, IoError> {
            Err(IoError::WouldBlock)
        }
    }

    #[test]
    fn emits_valid_post_header_and_json_base64_body() {
        let mut request = send_vision_request(b"a\"b", &[0xff, 0, 1]).unwrap();
        let mut socket = Capture(Vec::new());
        while request.poll_send(&mut socket).unwrap() != SendProgress::Complete {}
        let split = socket.0.windows(4).position(|window| window == b"\r\n\r\n").unwrap() + 4;
        assert!(socket.0.starts_with(b"POST /api/generate HTTP/1.1\r\nHost: 127.0.0.1:11434\r\n"));
        assert_eq!(&socket.0[split..], br#"{"model":"llama3.2-vision","prompt":"a\"b","images":["/wAB"],"stream":true}"#);
    }
}