import json
import unittest

from hyperos import PI, TAU, cos, encode_command, process_command, sin, transform
from hyperos.terminal_bridge import BinaryVector, VECTOR_BITS
from vm.svl_kernel import HyperOS


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


class HyperOSSimulationTests(unittest.TestCase):
    def test_commands_are_recorded_and_reported_without_execution(self):
        hyperos = HyperOS()
        first_pid = hyperos.exec_cmd("INIT")
        second_pid = hyperos.exec_cmd("RUN_HYPER")

        self.assertNotEqual(first_pid, second_pid)
        self.assertEqual(hyperos.proc[first_pid]["cmd"], "INIT")
        self.assertEqual(hyperos.proc[first_pid]["w"], 13)
        report = json.loads(hyperos.report())
        self.assertEqual(report["OS"], "8.1.0-Minified")
        self.assertEqual(report["Metric_C"], 2.06)
        self.assertEqual(len(report["Tasks"]), 2)

    def test_rejects_empty_nul_and_non_text_commands(self):
        hyperos = HyperOS()
        for command in ("", "  ", None, "invalid\x00command"):
            with self.subTest(command=command), self.assertRaises(ValueError):
                hyperos.exec_cmd(command)


if __name__ == "__main__":
    unittest.main()