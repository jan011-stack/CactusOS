//! Fixed-width command encoding and hyperspace-vector validation.

use sha2::{Digest, Sha256};

use crate::hyper_math::{hyper_sin, PI};

pub const VECTOR_BYTES: usize = 32;
pub const ROOT_TOKEN_HASH: [u8; VECTOR_BYTES] = [
    0x29, 0x89, 0x18, 0x41, 0x0a, 0xf0, 0xc1, 0x05, 0x8a, 0xf6, 0x42, 0xd6, 0x47, 0x46,
    0x3c, 0xdd, 0x2b, 0x64, 0x9d, 0x5c, 0xf6, 0x4c, 0x65, 0x21, 0xe6, 0x85, 0x74, 0xf3,
    0xea, 0x9a, 0x78, 0x51,
];

/// Stateless terminal bridge. It never executes command text.
pub struct HyperSpaceTerminal;

impl HyperSpaceTerminal {
    pub const fn new() -> Self {
        Self
    }

    /// Hash UTF-8 command text into a fixed 256-bit vector.
    pub fn encode_command(&self, command: &str) -> [u8; VECTOR_BYTES] {
        let digest = Sha256::digest(command.as_bytes());
        let mut vector = [0u8; VECTOR_BYTES];
        vector.copy_from_slice(&digest);
        vector
    }

    /// Apply the geometry-derived sparse byte transform to a vector.
    pub fn transform_to_hyperspace(
        &self,
        vector: [u8; VECTOR_BYTES],
    ) -> [u8; VECTOR_BYTES] {
        let mut output = [0u8; VECTOR_BYTES];
        self.transform_into(&vector, &mut output);
        output
    }

    /// Reject vectors that violate the reserved square-boundary constraint.
    pub fn try_transform_to_hyperspace(
        &self,
        vector: &[u8; VECTOR_BYTES],
        output: &mut [u8; VECTOR_BYTES],
    ) -> bool {
        if !self.validate_orthogonal(vector) {
            *output = [0; VECTOR_BYTES];
            return false;
        }
        self.transform_into(vector, output);
        true
    }

    /// Transform into caller-owned storage without heap allocation.
    pub fn transform_into(
        &self,
        vector: &[u8; VECTOR_BYTES],
        output: &mut [u8; VECTOR_BYTES],
    ) {
        for (index, slot) in output.iter_mut().enumerate() {
            let phase = (index & 3) as f64 * (PI / 2.0);
            let wave = hyper_sin(phase);
            let (stride, rotation) = if wave > 0.0 {
                (1usize, 1u32)
            } else if wave < 0.0 {
                (2usize, 2u32)
            } else {
                (4usize, 4u32)
            };
            let neighbor = (index + stride) & (VECTOR_BYTES - 1);
            *slot = vector[index] ^ vector[neighbor].rotate_left(rotation);
        }
        clear_square_boundary(output);
    }

    /// Validate a 16x16 bitmap whose one-cell square perimeter is reserved.
    /// Any set bit on the perimeter is rejected; interior bits are unrestricted.
    pub fn validate_orthogonal(&self, vector: &[u8; VECTOR_BYTES]) -> bool {
        for row in 0..16 {
            for column in 0..16 {
                if row != 0 && row != 15 && column != 0 && column != 15 {
                    continue;
                }
                let bit_index = row * 16 + column;
                let byte = vector[bit_index >> 3];
                let mask = 1u8 << (7 - (bit_index & 7));
                if byte & mask != 0 {
                    return false;
                }
            }
        }
        true
    }

    /// Encode, transform, then reject invalid vectors and clear the output.
    pub fn process_text_command(
        &self,
        command: &str,
        output: &mut [u8; VECTOR_BYTES],
    ) -> bool {
        let mut encoded = self.encode_command(command);
        clear_square_boundary(&mut encoded);
        if !self.try_transform_to_hyperspace(&encoded, output) {
            return false;
        }
        if self.validate_orthogonal(output) {
            true
        } else {
            *output = [0; VECTOR_BYTES];
            false
        }
    }

    /// Compare a supplied token to the SHA-256 digest of the specified phrase.
    /// The phrase is public by design here, so this is an API gate, not a secret.
    pub fn verify_root_token(&self, token: &[u8; VECTOR_BYTES]) -> bool {
        let mut difference = 0u8;
        for index in 0..VECTOR_BYTES {
            difference |= token[index] ^ ROOT_TOKEN_HASH[index];
        }
        difference == 0
    }
}

impl Default for HyperSpaceTerminal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{HyperSpaceTerminal, ROOT_TOKEN_HASH, VECTOR_BYTES};

    #[test]
    fn transform_is_fixed_width_and_deterministic() {
        let terminal = HyperSpaceTerminal::new();
        let input = terminal.encode_command("status");
        assert_eq!(terminal.transform_to_hyperspace(input), terminal.transform_to_hyperspace(input));
    }

    #[test]
    fn validation_rejects_bits_on_square_boundary() {
        let terminal = HyperSpaceTerminal::new();
        let mut vector = [0u8; VECTOR_BYTES];
        let mut output = [0xff; VECTOR_BYTES];
        assert!(terminal.validate_orthogonal(&vector));
        assert!(terminal.try_transform_to_hyperspace(&vector, &mut output));

        vector[0] = 0x80;
        assert!(!terminal.validate_orthogonal(&vector));
        assert!(!terminal.try_transform_to_hyperspace(&vector, &mut output));
        assert_eq!(output, [0; VECTOR_BYTES]);

        vector = [0; VECTOR_BYTES];
        vector[8] = 0x40;
        assert!(terminal.validate_orthogonal(&vector));
    }

    #[test]
    fn root_token_comparison_checks_the_full_digest() {
        let terminal = HyperSpaceTerminal::new();
        assert!(terminal.verify_root_token(&ROOT_TOKEN_HASH));

        let mut invalid = ROOT_TOKEN_HASH;
        invalid[31] ^= 1;
        assert!(!terminal.verify_root_token(&invalid));
    }

    #[test]
    fn command_processing_aligns_and_accepts_the_square_boundary() {
        let terminal = HyperSpaceTerminal::new();
        let mut output = [0u8; VECTOR_BYTES];
        assert!(terminal.process_text_command("status", &mut output));
        assert!(terminal.validate_orthogonal(&output));
    }
}

fn clear_square_boundary(vector: &mut [u8; VECTOR_BYTES]) {
    for row in 0..16 {
        for column in 0..16 {
            if row != 0 && row != 15 && column != 0 && column != 15 {
                continue;
            }
            let bit_index = row * 16 + column;
            vector[bit_index >> 3] &= !(1u8 << (7 - (bit_index & 7)));
        }
    }
}