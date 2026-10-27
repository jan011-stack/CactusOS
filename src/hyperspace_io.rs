//! Fixed-buffer UTF-8 input normalization and square-state transformation.

use crate::hyper_math::hyper_sin;
use crate::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

pub const MATRIX_SIDE: usize = 16;
pub type StateMatrix = [[i8; MATRIX_SIDE]; MATRIX_SIDE];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputError {
    TooLong,
    InvalidUtf8,
    InvalidSquareGeometry,
}

/// Platform code implements this hook to raise its input-fault interrupt.
pub trait InputFaultInterrupt {
    fn raise_input_fault(&self);
}

pub struct HyperSpaceIoCore<'a> {
    interrupt: &'a dyn InputFaultInterrupt,
}

impl<'a> HyperSpaceIoCore<'a> {
    pub const fn new(interrupt: &'a dyn InputFaultInterrupt) -> Self {
        Self { interrupt }
    }

    /// Normalize one UTF-8 byte stream into a 32-byte buffer and 16x16 matrix.
    /// Invalid input is erased before the platform interrupt hook is called.
    pub fn process_input(
        &self,
        input: &[u8],
        buffer: &mut [u8; VECTOR_BYTES],
        matrix: &mut StateMatrix,
    ) -> Result<(), InputError> {
        *buffer = [0; VECTOR_BYTES];
        *matrix = [[0; MATRIX_SIDE]; MATRIX_SIDE];

        if input.len() > VECTOR_BYTES {
            return self.purge_and_interrupt(buffer, matrix, InputError::TooLong);
        }
        if core::str::from_utf8(input).is_err() {
            return self.purge_and_interrupt(buffer, matrix, InputError::InvalidUtf8);
        }

        buffer[..input.len()].copy_from_slice(input);
        if !HyperSpaceTerminal::new().validate_orthogonal(buffer) {
            return self.purge_and_interrupt(
                buffer,
                matrix,
                InputError::InvalidSquareGeometry,
            );
        }

        for (index, bit) in (0..MATRIX_SIDE * MATRIX_SIDE).enumerate() {
            let byte = buffer[bit >> 3];
            let occupied = byte & (1u8 << (7 - (bit & 7))) != 0;
            if !occupied {
                continue;
            }
            let row = index / MATRIX_SIDE;
            let column = index % MATRIX_SIDE;
            let phase = ((row + column) & 3) as f64 * 2.0;
            let wave = hyper_sin(phase);
            matrix[row][column] = if wave > 0.0 { 1 } else if wave < 0.0 { -1 } else { 0 };
        }
        Ok(())
    }

    fn purge_and_interrupt(
        &self,
        buffer: &mut [u8; VECTOR_BYTES],
        matrix: &mut StateMatrix,
        error: InputError,
    ) -> Result<(), InputError> {
        *buffer = [0; VECTOR_BYTES];
        *matrix = [[0; MATRIX_SIDE]; MATRIX_SIDE];
        self.interrupt.raise_input_fault();
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{HyperSpaceIoCore, InputError, InputFaultInterrupt, StateMatrix};
    use crate::terminal::VECTOR_BYTES;
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct TestInterrupt(AtomicUsize);

    impl InputFaultInterrupt for TestInterrupt {
        fn raise_input_fault(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn valid_interior_input_maps_to_triangular_state() {
        let interrupt = TestInterrupt(AtomicUsize::new(0));
        let core = HyperSpaceIoCore::new(&interrupt);
        let mut input = [0; VECTOR_BYTES];
        input[17] = 0x40;
        let mut buffer = [0; VECTOR_BYTES];
        let mut matrix: StateMatrix = [[0; 16]; 16];

        assert_eq!(core.process_input(&input, &mut buffer, &mut matrix), Ok(()));
        assert_eq!(matrix[8][9], 1);
        assert_eq!(interrupt.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn invalid_geometry_is_purged_before_interrupt_hook() {
        let interrupt = TestInterrupt(AtomicUsize::new(0));
        let core = HyperSpaceIoCore::new(&interrupt);
        let input = [0x40];
        let mut buffer = [0xff; VECTOR_BYTES];
        let mut matrix: StateMatrix = [[1; 16]; 16];

        assert_eq!(
            core.process_input(&input, &mut buffer, &mut matrix),
            Err(InputError::InvalidSquareGeometry)
        );
        assert_eq!(buffer, [0; VECTOR_BYTES]);
        assert_eq!(matrix, [[0; 16]; 16]);
        assert_eq!(interrupt.0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn malformed_utf8_and_oversized_input_trigger_fault() {
        let interrupt = TestInterrupt(AtomicUsize::new(0));
        let core = HyperSpaceIoCore::new(&interrupt);
        let mut buffer = [0; VECTOR_BYTES];
        let mut matrix: StateMatrix = [[0; 16]; 16];

        assert_eq!(
            core.process_input(&[0xff], &mut buffer, &mut matrix),
            Err(InputError::InvalidUtf8)
        );
        assert_eq!(
            core.process_input(&[b'a'; VECTOR_BYTES + 1], &mut buffer, &mut matrix),
            Err(InputError::TooLong)
        );
        assert_eq!(interrupt.0.load(Ordering::Relaxed), 2);
    }
}