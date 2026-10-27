//! Allocation-free UTF-8 and model-control-marker filter for streamed text.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterError {
    InvalidUtf8,
    ControlByte,
    ModelControlMarker,
    AlreadyHalted,
}

pub struct HyperTokenFilter {
    continuation_left: u8,
    next_min: u8,
    next_max: u8,
    history: [u8; 12],
    history_len: usize,
    history_cursor: usize,
    halted: bool,
}

impl HyperTokenFilter {
    pub const fn new() -> Self {
        Self {
            continuation_left: 0,
            next_min: 0x80,
            next_max: 0xbf,
            history: [0; 12],
            history_len: 0,
            history_cursor: 0,
            halted: false,
        }
    }

    pub const fn is_halted(&self) -> bool {
        self.halted
    }

    pub fn finish(&mut self) -> bool {
        if self.continuation_left != 0 {
            self.halted = true;
            return false;
        }
        true
    }

    /// Sanitize a valid prefix in place. Failure volatile-zeroes the whole input slice.
    /// UTF-8 and marker state is preserved across calls.
    pub fn sanitize_in_place(
        &mut self,
        buffer: &mut [u8],
        length: &mut usize,
    ) -> Result<(), FilterError> {
        if self.halted {
            purge(buffer);
            *length = 0;
            return Err(FilterError::AlreadyHalted);
        }
        if *length > buffer.len() {
            self.halt(buffer, length);
            return Err(FilterError::ControlByte);
        }

        let input_len = *length;
        let mut write = 0usize;
        for read in 0..input_len {
            let byte = buffer[read];
            if let Err(error) = self.validate_byte(byte) {
                self.halt(buffer, length);
                return Err(error);
            }
            buffer[write] = byte;
            write += 1;
            self.remember(byte);
            if self.ends_in_model_control_marker() {
                self.halt(buffer, length);
                return Err(FilterError::ModelControlMarker);
            }
        }
        *length = write;
        Ok(())
    }

    fn validate_byte(&mut self, byte: u8) -> Result<(), FilterError> {
        if self.continuation_left != 0 {
            if byte < self.next_min || byte > self.next_max {
                return Err(FilterError::InvalidUtf8);
            }
            self.continuation_left -= 1;
            self.next_min = 0x80;
            self.next_max = 0xbf;
            return Ok(());
        }

        match byte {
            0x20..=0x7e | b'\n' | b'\r' | b'\t' => Ok(()),
            0xc2..=0xdf => {
                self.continuation_left = 1;
                Ok(())
            }
            0xe0 => {
                self.continuation_left = 2;
                self.next_min = 0xa0;
                Ok(())
            }
            0xe1..=0xec | 0xee..=0xef => {
                self.continuation_left = 2;
                Ok(())
            }
            0xed => {
                self.continuation_left = 2;
                self.next_max = 0x9f;
                Ok(())
            }
            0xf0 => {
                self.continuation_left = 3;
                self.next_min = 0x90;
                Ok(())
            }
            0xf1..=0xf3 => {
                self.continuation_left = 3;
                Ok(())
            }
            0xf4 => {
                self.continuation_left = 3;
                self.next_max = 0x8f;
                Ok(())
            }
            _ => Err(FilterError::ControlByte),
        }
    }

    fn remember(&mut self, byte: u8) {
        self.history[self.history_cursor] = byte;
        self.history_cursor = (self.history_cursor + 1) % self.history.len();
        self.history_len = (self.history_len + 1).min(self.history.len());
    }

    fn ends_in_model_control_marker(&self) -> bool {
        const MARKERS: [&[u8]; 5] = [
            b"<|im_start|>",
            b"<|im_end|>",
            b"<|system|>",
            b"<<SYS>>",
            b"[INST]",
        ];
        MARKERS.iter().any(|marker| {
            if marker.len() > self.history_len {
                return false;
            }
            marker.iter().enumerate().all(|(index, expected)| {
                let start = (self.history_cursor + self.history.len() - marker.len())
                    % self.history.len();
                self.history[(start + index) % self.history.len()] == *expected
            })
        })
    }

    fn halt(&mut self, buffer: &mut [u8], length: &mut usize) {
        purge(buffer);
        *length = 0;
        self.halted = true;
    }
}

impl Default for HyperTokenFilter {
    fn default() -> Self {
        Self::new()
    }
}

fn purge(buffer: &mut [u8]) {
    for byte in buffer {
        // SAFETY: the mutable slice is exclusively borrowed for this purge.
        unsafe { core::ptr::write_volatile(byte, 0) };
    }
}

#[cfg(test)]
mod tests {
    use super::{FilterError, HyperTokenFilter};

    #[test]
    fn accepts_utf8_split_across_input_chunks() {
        let mut filter = HyperTokenFilter::new();
        let mut first = [0xc5];
        let mut first_len = 1;
        assert_eq!(filter.sanitize_in_place(&mut first, &mut first_len), Ok(()));
        let mut second = [0x82, b'a'];
        let mut second_len = 2;
        assert_eq!(filter.sanitize_in_place(&mut second, &mut second_len), Ok(()));
        assert_eq!(&second[..second_len], &[0x82, b'a']);
    }

    #[test]
    fn control_bytes_and_split_model_markers_halt_and_purge() {
        let mut filter = HyperTokenFilter::new();
        let mut safe = *b"<|im_";
        let mut safe_len = safe.len();
        assert_eq!(filter.sanitize_in_place(&mut safe, &mut safe_len), Ok(()));
        let mut marker = *b"start|>";
        let mut marker_len = marker.len();
        assert_eq!(
            filter.sanitize_in_place(&mut marker, &mut marker_len),
            Err(FilterError::ModelControlMarker)
        );
        assert_eq!(marker, [0; 7]);
        assert!(filter.is_halted());

        let mut second = HyperTokenFilter::new();
        let mut escape = [0x1b, b'['];
        let mut escape_len = 2;
        assert_eq!(second.sanitize_in_place(&mut escape, &mut escape_len), Err(FilterError::ControlByte));
        assert_eq!(escape, [0; 2]);
    }
}