"""Encode text as 256-bit vectors and apply a fixed GF(2) transform.

This module does not execute the encoded text as a shell command.
"""

from dataclasses import dataclass
from hashlib import sha256

VECTOR_BITS = 256
BYTE_COUNT = VECTOR_BITS // 8
TRANSFORM_MATRIX = tuple(
    tuple(int(column == row or column == row - 1) for column in range(BYTE_COUNT))
    for row in range(BYTE_COUNT)
)


@dataclass(frozen=True)
class BinaryVector:
    """An immutable, fixed-width vector of binary values."""

    bits: tuple[int, ...]

    def __post_init__(self) -> None:
        if len(self.bits) != VECTOR_BITS or any(bit not in (0, 1) for bit in self.bits):
            raise ValueError(f"bits must contain exactly {VECTOR_BITS} binary values")


def encode_command(command: str) -> BinaryVector:
    """Return the SHA-256 fingerprint of UTF-8 command text as 256 bits."""
    digest = sha256(command.encode("utf-8")).digest()
    bits = tuple((byte >> shift) & 1 for byte in digest for shift in range(7, -1, -1))
    return BinaryVector(bits)


def transform(vector: BinaryVector) -> BinaryVector:
    """Apply a lower-bidiagonal 16x16 matrix over GF(2), byte-wise."""
    input_bytes = tuple(
        sum(vector.bits[offset + bit] << (7 - bit) for bit in range(8))
        for offset in range(0, VECTOR_BITS, 8)
    )
    output_bytes = tuple(
        _xor(input_bytes[column] for column, coefficient in enumerate(row) if coefficient)
        for row in TRANSFORM_MATRIX
    )
    output_bits = tuple(
        (byte >> shift) & 1 for byte in output_bytes for shift in range(7, -1, -1)
    )
    return BinaryVector(output_bits)


def process_command(command: str) -> BinaryVector:
    """Encode command text and return its transformed vector without execution."""
    return transform(encode_command(command))


def _xor(values: object) -> int:
    result = 0
    for value in values:
        result ^= value
    return result