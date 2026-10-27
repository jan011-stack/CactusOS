import unittest

from hyperos.agents import (
    AGENTS,
    CACTUS_SYSTEM_CONTEXT,
    DEFAULT_MODEL,
    MAX_HISTORY_MESSAGES,
    MODEL_OPTIONS,
    connect_agents,
    system_prompt,
)


class AgentChatTests(unittest.TestCase):
    def test_each_agent_uses_its_own_role_and_keeps_history(self):
        seen = []

        def transport(payload):
            seen.append(payload)
            return {"message": {"content": " odpowiedź "}}

        agents = connect_agents(transport=transport)
        self.assertEqual(set(agents), set(AGENTS))
        self.assertEqual(agents["astyria"].say("cześć"), "odpowiedź")
        agents["astyria"].say("jeszcze raz")

        self.assertIn("Astyria", seen[0]["messages"][0]["content"])
        self.assertEqual(len(seen[1]["messages"]), 4)
        self.assertEqual(agents["alex"].history, [])

    def test_failed_transport_preserves_history_and_retry_is_not_duplicated(self):
        seen = []

        def transport(payload):
            seen.append(payload)
            if len(seen) == 2:
                raise OSError("Backend unavailable")
            return {"message": {"content": "Gotowe"}}

        agent = connect_agents(transport=transport)["si_1"]
        agent.say("pierwsze pytanie")
        original = agent.history.copy()
        with self.assertRaises(OSError):
            agent.say("ponow pytanie")
        self.assertEqual(agent.history, original)
        agent.say("ponow pytanie")
        self.assertEqual(seen[1]["messages"], seen[2]["messages"])

    def test_malformed_reply_does_not_change_history(self):
        agent = connect_agents(transport=lambda payload: {"message": {}})["si_1"]
        with self.assertRaises(KeyError):
            agent.say("pytanie")
        self.assertEqual(agent.history, [])

    def test_history_limit_retains_complete_turns(self):
        seen = []

        def transport(payload):
            seen.append(payload)
            return {"message": {"content": "Gotowe"}}

        agent = connect_agents(transport=transport)["si_1"]
        for index in range(MAX_HISTORY_MESSAGES):
            agent.say(str(index))
            self.assertLessEqual(len(agent.history), MAX_HISTORY_MESSAGES)
            self.assertEqual([message["role"] for message in agent.history],
                             ["user", "assistant"] * (len(agent.history) // 2))
        self.assertEqual(len(agent.history), MAX_HISTORY_MESSAGES)
        for payload in seen:
            self.assertEqual(payload["messages"][1]["role"], "user")
            self.assertLessEqual(len(payload["messages"]), MAX_HISTORY_MESSAGES)

    def test_local_ai_profile_is_used_with_bounded_context(self):
        seen = []

        def transport(payload):
            seen.append(payload)
            return {"message": {"content": "Gotowe"}}

        connect_agents(transport=transport)["si_1"].say("czesc")
        self.assertEqual(DEFAULT_MODEL, "cactus-ai")
        self.assertEqual(seen[0]["model"], DEFAULT_MODEL)
        self.assertEqual(seen[0]["options"], MODEL_OPTIONS)
        self.assertEqual(seen[0]["options"]["num_ctx"], 512)
        self.assertEqual(seen[0]["options"]["num_batch"], 1)
        self.assertEqual(seen[0]["options"]["num_thread"], 1)
        self.assertEqual(seen[0]["options"]["num_predict"], 96)
        self.assertEqual(seen[0]["keep_alive"], 0)
        self.assertIn("nie są osobnymi modelami", CACTUS_SYSTEM_CONTEXT)
        self.assertIn("podaj nazwę aktywnego backendu", system_prompt(AGENTS["alex"]))
        self.assertIn("Brak potwierdzonego połączenia telemetrycznego", CACTUS_SYSTEM_CONTEXT)
        self.assertIn("nie jest uruchomionym systemem operacyjnym", seen[0]["messages"][0]["content"])


if __name__ == "__main__":
    unittest.main()
