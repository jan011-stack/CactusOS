import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from hyperos.vscode_integration import default_config_file, inject_si1_to_vscode, main


class VSCodeIntegrationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.config = Path(temporary.name) / "User/chatLanguageModels.json"

    def write_config(self, content):
        self.config.parent.mkdir(parents=True, exist_ok=True)
        self.config.write_text(content, encoding="utf-8")

    def test_creates_supported_provider_without_claiming_tools(self):
        inject_si1_to_vscode(self.config)
        group, = json.loads(self.config.read_text())
        self.assertEqual(group["vendor"], "customendpoint")
        model, = group["models"]
        self.assertEqual(model["id"], "cactus-ai:latest")
        self.assertEqual(model["url"], "http://127.0.0.1:11434/v1/chat/completions")
        self.assertFalse(model["toolCalling"])

    def test_preserves_other_providers_models_and_initial_backup(self):
        other = {"name": "Other", "vendor": "other", "settings": {"enabled": True}}
        initial = json.dumps([other, {"name": "SI_1", "vendor": "customendpoint",
                                    "models": [{"id": "other-model"}], "settings": {"keep": True}}])
        self.write_config(initial)
        inject_si1_to_vscode(self.config)
        result = json.loads(self.config.read_text())
        self.assertEqual(result[0], other)
        self.assertEqual(result[1]["models"][0], {"id": "other-model"})
        self.assertEqual(result[1]["settings"], {"keep": True})
        self.assertEqual(self.config.with_suffix(".json.bak").read_text(), initial)
        with patch("hyperos.vscode_integration.os.replace") as replace:
            inject_si1_to_vscode(self.config)
            replace.assert_not_called()

    def test_updates_existing_model_without_losing_custom_fields(self):
        self.write_config(json.dumps([{"name": "SI_1", "vendor": "customendpoint", "models": [
            {"id": "cactus-ai:latest", "url": "http://old", "requestHeaders": {"X-Test": "yes"}},
        ]}]))
        inject_si1_to_vscode(self.config, tool_calling=True)
        model, = json.loads(self.config.read_text())[0]["models"]
        self.assertTrue(model["toolCalling"])
        self.assertEqual(model["requestHeaders"], {"X-Test": "yes"})

    def test_invalid_existing_config_is_never_overwritten(self):
        for content in ("invalid", "{}", "[null]", '[{"name":"SI_1","vendor":"customendpoint","models":{}}]'):
            with self.subTest(content=content):
                self.write_config(content)
                with self.assertRaises(ValueError):
                    inject_si1_to_vscode(self.config)
                self.assertEqual(self.config.read_text(), content)

    def test_rejects_bad_endpoint_and_limits_before_writing(self):
        for options in ({"url": "file:///tmp/chat/completions"}, {"max_input_tokens": 0},
                        {"url": "http://user:secret@localhost/v1/chat/completions"},
                        {"url": "http://localhost:65536/v1/chat/completions"},
                        {"url": "http://localhost:invalid/v1/chat/completions"},
                        {"url": "http://localhost:0/v1/chat/completions"},
                        {"url": "http://local\nhost/v1/chat/completions"},
                        {"url": "http://local host/v1/chat/completions"},
                        {"url": "http://@localhost/v1/chat/completions"},
                        {"model": " "}, {"max_output_tokens": -1}):
            with self.subTest(options=options), self.assertRaises(ValueError):
                inject_si1_to_vscode(self.config, **options)
        self.assertFalse(self.config.exists())

    def test_failed_replace_leaves_original_and_cleans_temporary_file(self):
        self.write_config("[]")
        with patch("hyperos.vscode_integration.os.replace", side_effect=OSError("test")):
            with self.assertRaises(OSError):
                inject_si1_to_vscode(self.config)
        self.assertEqual(self.config.read_text(), "[]")
        self.assertEqual(sorted(path.name for path in self.config.parent.iterdir()),
                         ["chatLanguageModels.json", "chatLanguageModels.json.bak"])

    def test_desktop_paths(self):
        with patch("hyperos.vscode_integration.platform.system", return_value="Darwin"):
            self.assertEqual(default_config_file(), Path.home() / "Library/Application Support/Code/User/chatLanguageModels.json")
        with patch("hyperos.vscode_integration.platform.system", return_value="Linux"), patch.dict(os.environ, {"XDG_CONFIG_HOME": "/tmp/config"}):
            self.assertEqual(default_config_file(), Path("/tmp/config/Code/User/chatLanguageModels.json"))
        with patch("hyperos.vscode_integration.platform.system", return_value="Windows"), patch.dict(os.environ, {"APPDATA": "/tmp/appdata"}):
            self.assertEqual(default_config_file(), Path("/tmp/appdata/Code/User/chatLanguageModels.json"))

    def test_remote_requires_explicit_profile(self):
        with patch.dict(os.environ, {"CODESPACES": "true"}), patch("hyperos.vscode_integration.inject_si1_to_vscode") as inject:
            with self.assertRaises(SystemExit) as result:
                main([])
            self.assertEqual(result.exception.code, 2)
            inject.assert_not_called()


if __name__ == "__main__":
    unittest.main()