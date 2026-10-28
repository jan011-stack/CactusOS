import unittest

from hyperos.model_profiles import (
    CACTUS_SYSTEM_CONTEXT,
    DEFAULT_MODEL,
    MODEL_OPTIONS,
    active_profile,
    model_options,
)


class ModelProfileTests(unittest.TestCase):
    def test_default_profile_is_eco_with_bounded_context(self):
        self.assertEqual(DEFAULT_MODEL, "llama3.1:8b")
        self.assertEqual(MODEL_OPTIONS, model_options())
        self.assertEqual(MODEL_OPTIONS["num_ctx"], 2048)
        self.assertEqual(MODEL_OPTIONS["num_batch"], 64)
        self.assertEqual(MODEL_OPTIONS["num_thread"], 4)
        self.assertEqual(MODEL_OPTIONS["num_predict"], 1024)
        self.assertEqual(active_profile()["keep_alive"], "5m")

    def test_system_context_states_limits(self):
        self.assertIn("Brak potwierdzonego połączenia telemetrycznego", CACTUS_SYSTEM_CONTEXT)
        self.assertIn("nie jest uruchomionym systemem operacyjnym", CACTUS_SYSTEM_CONTEXT)


if __name__ == "__main__":
    unittest.main()
