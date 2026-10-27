import unittest
from unittest.mock import patch

from hyperos.si1_agent import propose_with_si1, run_mission
from hyperos.web_server import TerminalBroker, execute_command


class SI1AgentTests(unittest.TestCase):
    def setUp(self):
        self.executed = []
        self.prompts = []
        self.output = []

    def make_broker(self, exit_code=0):
        def model(prompt):
            self.prompts.append(prompt)
            if len(self.prompts) == 1:
                return {"reply": "Sprawdze katalog", "command": "pwd"}
            return {"reply": "Zadanie zakonczone", "command": None}

        def executor(command):
            self.executed.append(command)
            return {"exit_code": exit_code, "output": "/workspaces/CactusOS\n", "truncated": False}

        return TerminalBroker(model=model, executor=executor)

    def test_command_waits_for_exact_approval_and_result_returns_to_model(self):
        def approve(prompt):
            self.assertEqual(self.executed, [])
            self.assertIn("pwd", self.output[-1])
            return "WYKONAJ"

        run_mission("Sprawdz katalog", self.make_broker(), read=approve, write=self.output.append)
        self.assertEqual(self.executed, ["pwd"])
        self.assertIn("/workspaces/CactusOS", self.prompts[1])
        self.assertIn('"exit_code": 0', self.prompts[1])

    def test_cancellation_never_executes(self):
        for answer in ("", "tak", "wykonaj"):
            with self.subTest(answer=answer):
                self.prompts.clear()
                run_mission("Sprawdz katalog", self.make_broker(), read=lambda prompt: answer,
                            write=self.output.append)
        self.assertEqual(self.executed, [])

    def test_failure_stops_mission(self):
        run_mission("Sprawdz katalog", self.make_broker(exit_code=1),
                    read=lambda prompt: "WYKONAJ", write=self.output.append)
        self.assertEqual(len(self.prompts), 1)
        self.assertIn("nie powiodlo", self.output[-1])

    def test_step_limit_stops_without_another_model_call(self):
        run_mission("Sprawdz katalog", self.make_broker(), max_steps=1,
                    read=lambda prompt: "WYKONAJ", write=self.output.append)
        self.assertEqual(len(self.prompts), 1)
        self.assertIn("limit krokow", self.output[-1])

    def test_invalid_mission_or_limits_do_not_call_model(self):
        for mission, steps in (("", 1), ("x" * 801, 1), ("test", 0), ("test", 11)):
            with self.subTest(mission=mission, steps=steps), self.assertRaises(ValueError):
                run_mission(mission, self.make_broker(), max_steps=steps)
        self.assertEqual(self.prompts, [])

    def test_si1_profile_is_selected_with_non_root_instructions(self):
        with patch("hyperos.si1_agent.propose_with_primary_model", return_value={"reply": "OK", "command": None}) as model:
            propose_with_si1("Sprawdz katalog")
        self.assertEqual(model.call_args.kwargs["model"], "si_1")
        self.assertEqual(model.call_args.args[0], "Sprawdz katalog")
        self.assertIn("nie root", model.call_args.kwargs["system"])

    def test_si1_uses_gemini_then_falls_back_to_local_si1_profile(self):
        from urllib.error import URLError
        reply = {"reply": "OK", "command": None}
        with patch("hyperos.web_server.gemini_api_key", return_value="test-key"), \
                patch("hyperos.web_server.propose_with_gemini", return_value=reply) as gemini, \
                patch("hyperos.web_server.propose_with_ollama") as ollama:
            self.assertEqual(propose_with_si1("Sprawdz katalog"), reply)
        self.assertIn("SI_1", gemini.call_args.kwargs["system"])
        ollama.assert_not_called()
        with patch("hyperos.web_server.gemini_api_key", return_value="test-key"), \
                patch("hyperos.web_server.propose_with_gemini", side_effect=URLError("offline")), \
                patch("hyperos.web_server.propose_with_ollama", return_value=reply) as ollama:
            self.assertEqual(propose_with_si1("Sprawdz katalog"), reply)
        self.assertEqual(ollama.call_args.kwargs["model"], "si_1")

    def test_executor_runs_real_command_without_sudo(self):
        result = execute_command("printf si1-test")
        self.assertEqual(result, {"exit_code": 0, "output": "si1-test", "truncated": False})


if __name__ == "__main__":
    unittest.main()