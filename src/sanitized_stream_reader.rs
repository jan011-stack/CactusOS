//! Bounded NDJSON response parser for Ollama streaming output.

pub const MAX_JSON_LINE: usize = 1024;
pub const STATUS_MALFORMED: u8 = 1 << 0;
pub const STATUS_OVERFLOW: u8 = 1 << 1;
pub const STATUS_DONE: u8 = 1 << 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamError {
    Malformed,
    LineOverflow,
    OutputOverflow,
}

pub struct SanitizedStreamReader<'a> {
    output: &'a mut [u8],
    output_len: usize,
    line: [u8; MAX_JSON_LINE],
    line_len: usize,
    discarding_line: bool,
    status: u8,
}

impl<'a> SanitizedStreamReader<'a> {
    pub fn new(output: &'a mut [u8]) -> Self {
        Self {
            output,
            output_len: 0,
            line: [0; MAX_JSON_LINE],
            line_len: 0,
            discarding_line: false,
            status: 0,
        }
    }

    /// Consume arbitrary socket chunks. Bad/oversized lines are discarded safely.
    pub fn push(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                if self.discarding_line {
                    self.status |= STATUS_OVERFLOW;
                } else if self.line_len != 0 {
                    let line_len = if self.line_len > 0 && self.line[self.line_len - 1] == b'\r' {
                        self.line_len - 1
                    } else {
                        self.line_len
                    };
                    let mut position = 0;
                    let output_before = self.output_len;
                    let status_before = self.status;
                    if !parse_record(
                        &self.line[..line_len],
                        &mut position,
                        &mut self.output[..],
                        &mut self.output_len,
                        &mut self.status,
                    ) {
                        self.output_len = output_before;
                        self.status = status_before;
                        self.status |= STATUS_MALFORMED;
                    }
                }
                self.line_len = 0;
                self.discarding_line = false;
            } else if !self.discarding_line {
                if self.line_len == MAX_JSON_LINE {
                    self.line_len = 0;
                    self.discarding_line = true;
                } else {
                    self.line[self.line_len] = byte;
                    self.line_len += 1;
                }
            }
        }
    }

    pub fn output(&self) -> &[u8] {
        &self.output[..self.output_len]
    }

    /// Copy and remove up to `destination.len()` parsed bytes from the front.
    pub fn take_output(&mut self, destination: &mut [u8]) -> usize {
        let count = self.output_len.min(destination.len());
        destination[..count].copy_from_slice(&self.output[..count]);
        self.output.copy_within(count..self.output_len, 0);
        self.output_len -= count;
        count
    }

    pub const fn output_len(&self) -> usize {
        self.output_len
    }

    pub const fn status_flags(&self) -> u8 {
        self.status
    }

    pub fn take_status(&mut self) -> u8 {
        let status = self.status;
        self.status = 0;
        status
    }

    pub fn is_done(&self) -> bool {
        self.status & STATUS_DONE != 0
    }

    /// Mark a truncated final NDJSON record when the HTTP stream closes.
    pub fn finish(&mut self) {
        if self.discarding_line {
            self.status |= STATUS_OVERFLOW;
            self.discarding_line = false;
        } else if self.line_len != 0 {
            self.status |= STATUS_MALFORMED;
        }
        self.line_len = 0;
    }

    pub fn reset(&mut self) {
        self.output_len = 0;
        self.line_len = 0;
        self.discarding_line = false;
        self.status = 0;
    }

    /// Volatile-zero both parser buffers while preserving diagnostic status flags.
    pub fn purge_buffers(&mut self) {
        for byte in self.output.iter_mut() {
            // SAFETY: this reader holds exclusive access to its backing buffer.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        for byte in self.line.iter_mut() {
            // SAFETY: this reader holds exclusive access to its line buffer.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        self.output_len = 0;
        self.line_len = 0;
        self.discarding_line = false;
    }
}

fn parse_record(
    input: &[u8],
    position: &mut usize,
    output: &mut [u8],
    output_len: &mut usize,
    status: &mut u8,
) -> bool {
    if core::str::from_utf8(input).is_err() {
        return false;
    }
    skip_ws(input, position);
    if take(input, position) != Some(b'{') {
        return false;
    }
    skip_ws(input, position);
    if peek(input, *position) == Some(b'}') {
        *position += 1;
        skip_ws(input, position);
        return *position == input.len();
    }

    loop {
        let mut key = [0u8; 32];
        let mut key_len = 0;
        if !read_json_string(input, position, &mut key, &mut key_len, None) {
            return false;
        }
        skip_ws(input, position);
        if take(input, position) != Some(b':') {
            return false;
        }
        skip_ws(input, position);

        if &key[..key_len] == b"response" {
            let before = *output_len;
            if !read_json_string(input, position, output, output_len, Some(status)) {
                return false;
            }
            if *output_len == before && output.is_empty() {
                *status |= STATUS_OVERFLOW;
            }
        } else if &key[..key_len] == b"done" {
            let value_start = *position;
            if input.get(*position..*position + 4) == Some(b"true") {
                *position += 4;
                *status |= STATUS_DONE;
            } else if input.get(*position..*position + 5) == Some(b"false") {
                *position += 5;
            } else {
                return false;
            }
            if *position == value_start {
                return false;
            }
        } else if !skip_value(input, position) {
            return false;
        }

        skip_ws(input, position);
        match take(input, position) {
            Some(b',') => skip_ws(input, position),
            Some(b'}') => {
                skip_ws(input, position);
                return *position == input.len();
            }
            _ => return false,
        }
    }
}

fn read_json_string(
    input: &[u8],
    position: &mut usize,
    output: &mut [u8],
    output_len: &mut usize,
    mut status: Option<&mut u8>,
) -> bool {
    if take(input, position) != Some(b'"') {
        return false;
    }
    loop {
        let Some(byte) = take(input, position) else {
            return false;
        };
        match byte {
            b'"' => return true,
            b'\\' => {
                let Some(escaped) = take(input, position) else {
                    return false;
                };
                let decoded = match escaped {
                    b'"' => b'"' as u32,
                    b'\\' => b'\\' as u32,
                    b'/' => b'/' as u32,
                    b'b' => 8,
                    b'f' => 12,
                    b'n' => b'\n' as u32,
                    b'r' => b'\r' as u32,
                    b't' => b'\t' as u32,
                    b'u' => {
                        let Some(codepoint) = read_unicode_escape(input, position) else {
                            return false;
                        };
                        codepoint
                    }
                    _ => return false,
                };
                if !append_codepoint(decoded, output, output_len, status.as_deref_mut()) {
                    return false;
                }
            }
            0..=0x1f => return false,
            0x80..=0xff if status.is_some() => {
                if !append_raw_byte(byte, output, output_len, status.as_deref_mut()) {
                    return false;
                }
            }
            value => {
                if !append_codepoint(value as u32, output, output_len, status.as_deref_mut()) {
                    return false;
                }
            }
        }
    }
}

fn append_raw_byte(
    byte: u8,
    output: &mut [u8],
    output_len: &mut usize,
    status: Option<&mut u8>,
) -> bool {
    if *output_len >= output.len() {
        if let Some(flags) = status {
            *flags |= STATUS_OVERFLOW;
            return true;
        }
        return false;
    }
    output[*output_len] = byte;
    *output_len += 1;
    true
}

fn read_unicode_escape(input: &[u8], position: &mut usize) -> Option<u32> {
    let first = read_hex_quad(input, position)?;
    if (0xd800..=0xdbff).contains(&first) {
        if take(input, position) != Some(b'\\') || take(input, position) != Some(b'u') {
            return None;
        }
        let second = read_hex_quad(input, position)?;
        if !(0xdc00..=0xdfff).contains(&second) {
            return None;
        }
        Some(0x10000 + (((first - 0xd800) << 10) | (second - 0xdc00)))
    } else if (0xdc00..=0xdfff).contains(&first) {
        None
    } else {
        Some(first)
    }
}

fn read_hex_quad(input: &[u8], position: &mut usize) -> Option<u32> {
    let mut value = 0u32;
    for _ in 0..4 {
        let digit = match take(input, position)? {
            digit @ b'0'..=b'9' => (digit - b'0') as u32,
            digit @ b'a'..=b'f' => (digit - b'a' + 10) as u32,
            digit @ b'A'..=b'F' => (digit - b'A' + 10) as u32,
            _ => return None,
        };
        value = (value << 4) | digit;
    }
    Some(value)
}

fn append_codepoint(
    codepoint: u32,
    output: &mut [u8],
    output_len: &mut usize,
    mut status: Option<&mut u8>,
) -> bool {
    let mut encoded = [0u8; 4];
    let length = if codepoint <= 0x7f {
        encoded[0] = codepoint as u8;
        1
    } else if codepoint <= 0x7ff {
        encoded[0] = (0xc0 | (codepoint >> 6)) as u8;
        encoded[1] = (0x80 | (codepoint & 0x3f)) as u8;
        2
    } else if codepoint <= 0xffff {
        encoded[0] = (0xe0 | (codepoint >> 12)) as u8;
        encoded[1] = (0x80 | ((codepoint >> 6) & 0x3f)) as u8;
        encoded[2] = (0x80 | (codepoint & 0x3f)) as u8;
        3
    } else if codepoint <= 0x10ffff {
        encoded[0] = (0xf0 | (codepoint >> 18)) as u8;
        encoded[1] = (0x80 | ((codepoint >> 12) & 0x3f)) as u8;
        encoded[2] = (0x80 | ((codepoint >> 6) & 0x3f)) as u8;
        encoded[3] = (0x80 | (codepoint & 0x3f)) as u8;
        4
    } else {
        return false;
    };

    let Some(end) = output_len.checked_add(length) else {
        if let Some(flags) = status.as_deref_mut() {
            *flags |= STATUS_OVERFLOW;
            return true;
        }
        return false;
    };
    if end > output.len() {
        if let Some(flags) = status.as_deref_mut() {
            *flags |= STATUS_OVERFLOW;
            return true;
        }
        return false;
    }
    output[*output_len..*output_len + length].copy_from_slice(&encoded[..length]);
    *output_len += length;
    true
}

fn skip_value(input: &[u8], position: &mut usize) -> bool {
    if peek(input, *position) == Some(b'"') {
        return skip_json_string(input, position);
    }
    let start = *position;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    while let Some(&byte) = input.get(*position) {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' if depth > 0 => depth -= 1,
                b',' | b'}' if depth == 0 => break,
                0..=0x20 => break,
                _ => {}
            }
        }
        *position += 1;
    }
    *position > start && !in_string && !escaped && depth == 0
}

fn skip_json_string(input: &[u8], position: &mut usize) -> bool {
    if take(input, position) != Some(b'"') {
        return false;
    }
    loop {
        let Some(byte) = take(input, position) else {
            return false;
        };
        match byte {
            b'"' => return true,
            b'\\' => match take(input, position) {
                Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {}
                Some(b'u') if read_unicode_escape(input, position).is_some() => {}
                _ => return false,
            },
            0..=0x1f => return false,
            _ => {}
        }
    }
}

fn skip_ws(input: &[u8], position: &mut usize) {
    while matches!(input.get(*position), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        *position += 1;
    }
}

fn take(input: &[u8], position: &mut usize) -> Option<u8> {
    let byte = *input.get(*position)?;
    *position += 1;
    Some(byte)
}

fn peek(input: &[u8], position: usize) -> Option<u8> {
    input.get(position).copied()
}

#[cfg(test)]
mod tests {
    use super::{SanitizedStreamReader, STATUS_DONE, STATUS_MALFORMED, STATUS_OVERFLOW};

    #[test]
    fn extracts_response_across_chunks_and_decodes_escapes() {
        let mut output = [0; 32];
        let mut reader = SanitizedStreamReader::new(&mut output);
        reader.push(br#"{"response":"hello \u263a"}"#);
        reader.push(b"\n{\"response\":\"!\",\"done\":true}\n");
        assert_eq!(reader.output(), "hello ☺!".as_bytes());
        assert_ne!(reader.status_flags() & STATUS_DONE, 0);
        assert_eq!(reader.status_flags() & STATUS_MALFORMED, 0);
    }

    #[test]
    fn preserves_raw_utf8_response_bytes() {
        let mut output = [0; 16];
        let mut reader = SanitizedStreamReader::new(&mut output);
        reader.push("{\"response\":\"zażółć\"}\n".as_bytes());
        assert_eq!(reader.output(), "zażółć".as_bytes());
    }

    #[test]
    fn take_output_drains_in_bounded_chunks() {
        let mut output = [0; 8];
        let mut reader = SanitizedStreamReader::new(&mut output);
        reader.push(b"{\"response\":\"abcd\"}\n");
        let mut first = [0; 2];
        let mut second = [0; 4];
        assert_eq!(reader.take_output(&mut first), 2);
        assert_eq!(&first, b"ab");
        assert_eq!(reader.take_output(&mut second), 2);
        assert_eq!(&second[..2], b"cd");
        assert!(reader.output().is_empty());
    }

    #[test]
    fn malformed_and_overflow_are_status_flags_without_panics() {
        let mut output = [0; 2];
        let mut reader = SanitizedStreamReader::new(&mut output);
        reader.push(b"not-json\n{\"response\":\"long\"}\n");
        assert_eq!(reader.output(), b"lo");
        assert_ne!(reader.status_flags() & STATUS_MALFORMED, 0);
        assert_ne!(reader.status_flags() & STATUS_OVERFLOW, 0);
    }
}