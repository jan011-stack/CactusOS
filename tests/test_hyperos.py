import unittest

from hyperos import PI, TAU, cos, encode_command, process_command, sin, transform
from hyperos.terminal_bridge import BinaryVector, VECTOR_BITS


class HyperMathTests(unittest.TestCase):
    def test_pi_period_and_triangle_wave_landmarks(self):
        self.assertEqual(PI, 4.0)
        self.assertEqual(TAU, 8.0)
        self.assertEqual([sin(point) for point in (0, 2, 4, 6, 8)], [0, 1, 0, -1, 0])
        self.assertEqual([cos(point) for point in (0, 2, 4, 6)], [1, 0, -1, 0])


class TerminalBridgeTests(unittest.TestCase):
    def test_encoding_is_fixed_width_and_deterministic(self):
        first = encode_command("status")
        second = encode_command("status")
        self.assertEqual(len(first.bits), VECTOR_BITS)
        self.assertEqual(first, second)

    def test_transform_and_processing_return_fixed_width_vectors(self):
        encoded = encode_command("status")
        self.assertEqual(len(transform(encoded).bits), VECTOR_BITS)
        self.assertEqual(process_command("status"), transform(encoded))

    def test_vector_rejects_invalid_width_or_values(self):
        with self.assertRaises(ValueError):
            BinaryVector((0, 1))
        with self.assertRaises(ValueError):
            BinaryVector((2,) * VECTOR_BITS)


if __name__ == "__main__":
    unittest.main()