import base64
import http.client
import io
import json
import os
import threading
import time
import unittest
from unittest.mock import patch
from http.server import ThreadingHTTPServer
from urllib.error import URLError
from urllib.error import HTTPError

from hyperos.web_server import (
    GEMINI_MODEL,
    TerminalBroker,
    _bounded_ping_command,
    _requests_execution,
    CACTUS_SI_MODEL,
    cactus_si_generate,
    execute_root,
    execute_user_command,
    gemini_generate,
    validate_chat_messages,
    load_dotenv,
    make_handler,
    propose_with_primary_model,
    propose_with_ollama,
    repo_change_response,
    root_available,
)
from hyperos.repository_changes import RepositoryChangeBroker


class TerminalBrokerTests(unittest.TestCase):
    def setUp(self):
        cactus_si_env = patch.dict("os.environ", {
            "CACTUS_SI_API_URL": "",
            "CACTUS_SI_API_KEY": "",
        })
        cactus_si_env.start()
        self.addCleanup(cactus_si_env.stop)
        self.executed = []
        self.executed_user = []
        self.broker = TerminalBroker(
            model=lambda prompt: {"reply": "Sprawdz UID", "command": "id -u"},
            executor=lambda command: self.executed.append(command) or {"exit_code": 0},
            user_executor=lambda command: self.executed_user.append(command) or {"exit_code": 0},
        )

    def test_proposal_does_not_execute_and_vector_is_256_bits(self):
        proposal = self.broker.propose("jaki mam uid?")
        self.assertEqual(self.executed, [])
        self.assertEqual(len(proposal["vector"]), 256)
        self.assertLessEqual(set(proposal["vector"]), {"0", "1"})

    def test_approval_executes_exactly_once(self):
        proposal = self.broker.propose("wykonaj id -u")
        self.broker.resolve(proposal["proposal_id"], True, "id -u")
        self.assertEqual(self.executed, ["id -u"])
        with self.assertRaises(ValueError):
            self.broker.resolve(proposal["proposal_id"], True, "id -u")

    def test_user_terminal_executes_as_user_and_cannot_resolve_root_proposal(self):
        user_proposal = self.broker.propose("wykonaj id -u", elevated=False)
        with self.assertRaises(ValueError):
            self.broker.resolve(user_proposal["proposal_id"], True, "id -u", elevated=True)
        self.broker.resolve(user_proposal["proposal_id"], True, "id -u", elevated=False)
        self.assertEqual(self.executed, [])
        self.assertEqual(self.executed_user, ["id -u"])

        root_proposal = self.broker.propose("wykonaj id -u")
        with self.assertRaises(ValueError):
            self.broker.resolve(root_proposal["proposal_id"], True, "id -u", elevated=False)
        self.broker.resolve(root_proposal["proposal_id"], True, "id -u", elevated=True)
        self.assertEqual(self.executed, ["id -u"])
        self.assertEqual(self.executed_user, ["id -u"])

    def test_user_executor_refuses_a_root_server_process(self):
        with patch("hyperos.web_server.os.geteuid", return_value=0):
            with self.assertRaises(PermissionError):
                execute_user_command("id -u")

    def test_root_process_can_accept_root_execution_without_sudo(self):
        with patch("hyperos.web_server.os.geteuid", return_value=0), patch(
                "hyperos.web_server.subprocess.run") as run:
            self.assertTrue(root_available())
            run.assert_not_called()

            with patch("hyperos.web_server.subprocess.Popen",
                       side_effect=RuntimeError("captured")) as popen:
                with self.assertRaisesRegex(RuntimeError, "captured"):
                    execute_root("id -u")
            self.assertEqual(popen.call_args.args[0][0], "/usr/bin/timeout")

    def test_cancel_tamper_and_expiry_never_execute(self):
        proposal = self.broker.propose("wykonaj id -u")
        self.broker.resolve(proposal["proposal_id"], False, "id -u")
        proposal = self.broker.propose("wykonaj id -u")
        with self.assertRaises(ValueError):
            self.broker.resolve(proposal["proposal_id"], True, "whoami")
        proposal = self.broker.propose("wykonaj id -u")
        self.broker.pending[proposal["proposal_id"]] = ("id -u", time.monotonic() - 1)
        with self.assertRaises(ValueError):
            self.broker.resolve(proposal["proposal_id"], True, "id -u")
        self.assertEqual(self.executed, [])

    def test_conversation_cannot_create_execution(self):
        self.broker.model = lambda prompt: {"reply": "Czesc", "command": None}
        proposal = self.broker.propose("czesc")
        self.assertIsNone(proposal["proposal_id"])
        self.assertEqual(self.executed, [])

    def test_explicit_intent_gate_blocks_model_command_on_questions(self):
        broker = TerminalBroker(
            model=lambda prompt: {"reply": "Tak", "command": "answer"},
            executor=lambda command: self.executed.append(command),
        )
        result = broker.propose("Czy agenci korzystaja z jednego modelu AI?")
        self.assertEqual(result["reply"], "Tak")
        self.assertIsNone(result["command"])
        self.assertIsNone(result["proposal_id"])
        self.assertEqual(self.executed, [])

    def test_polish_execution_intent_is_recognized(self):
        for prompt in (
            "Wyślij ping do localhost", "Sprawdź pliki w src", "Kazałem wysłać ping",
            "Uruchom testy Cargo", "Ulepsz system", "Run tests", "Build the project",
            "Połącz z siecią deep space NETWORK USA", "Connect to private network",
            "ls -la", "git status", "python3 -m unittest",
        ):
            self.assertTrue(_requests_execution(prompt), prompt)
        for prompt in (
            "Czy możesz wysłać ping?", "Co robi ping?", "Jak działa Cargo?",
            "Jak połączyć się z prywatną siecią?", "What does this test do?",
            "How to build this project?",
        ):
            self.assertFalse(_requests_execution(prompt), prompt)

    def test_ping_command_is_bounded_and_target_is_validated(self):
        command = _bounded_ping_command("Kazałem wysłać ping 127.0.0.1")
        self.assertIsNotNone(command)
        self.assertIn("-c 3 -W 2 127.0.0.1", command)
        self.assertIsNone(_bounded_ping_command("wyślij ping ; rm -rf /"))

    def test_nonstandard_json_text_is_reply_not_executable_command(self):
        broker = TerminalBroker(
            model=lambda prompt: {"Tak": "Wszyscy korzystają z jednego modelu cactus-ai."},
            executor=lambda command: self.executed.append(command),
        )
        proposal = broker.propose("czy agenci korzystają z jednego modelu?")
        self.assertEqual(proposal["reply"], "Wszyscy korzystają z jednego modelu cactus-ai.")
        self.assertIsNone(proposal["command"])
        self.assertIsNone(proposal["proposal_id"])
        self.assertEqual(self.executed, [])

    def test_model_receives_actual_ping_tool_and_approval_constraints(self):
        response = json.dumps({"message": {"content": json.dumps({
            "reply": "Propozycja", "command": "/opt/ping -n -c 3 -W 2 127.0.0.1",
        })}}).encode()
        with patch("hyperos.web_server.shutil.which", return_value="/opt/ping"), patch(
                "hyperos.web_server.urlopen", return_value=io.BytesIO(response)) as upstream:
            result = propose_with_ollama("wyslij ping 127.0.0.1")
        payload = json.loads(upstream.call_args.args[0].data)
        context = payload["messages"][0]["content"]
        self.assertIn("/opt/ping", context)
        self.assertIn("-n -c 3 -W 2", context)
        self.assertIn("zatwierdzeniu", context)
        self.assertEqual(result["command"], "/opt/ping -n -c 3 -W 2 127.0.0.1")
        self.assertEqual(self.executed, [])

    def test_gemini_request_uses_server_key_and_expected_model(self):
        image = "data:image/jpeg;base64," + base64.b64encode(b"\xff\xd8\xfftest").decode()
        response = io.BytesIO(json.dumps({
            "candidates": [{"content": {"parts": [{"text": "Cześć"}]}}],
        }).encode())
        with patch.dict("os.environ", {"GEMINI_API_KEY": "test-key"}), patch(
                "hyperos.web_server.urlopen", return_value=response) as upstream:
            answer = gemini_generate([
                {"role": "system", "content": "system"},
                {"role": "user", "content": "hello", "images": [{
                    "mime_type": "image/jpeg",
                    "data": base64.b64encode(b"\xff\xd8\xfftest").decode(),
                }]},
            ])
        request = upstream.call_args.args[0]
        payload = json.loads(request.data)
        self.assertEqual(answer, "Cześć")
        self.assertIn(GEMINI_MODEL, request.full_url)
        self.assertEqual(request.get_header("X-goog-api-key"), "test-key")
        self.assertEqual(payload["systemInstruction"]["parts"][0]["text"], "system")
        self.assertEqual(payload["contents"][0]["parts"][0]["text"], "hello")
        self.assertEqual(payload["contents"][0]["parts"][1], {
            "inlineData": {"mimeType": "image/jpeg", "data": base64.b64encode(b"\xff\xd8\xfftest").decode()},
        })

    def test_cactus_si_request_uses_server_key_and_chat_completion_payload(self):
        response = io.BytesIO(json.dumps({
            "choices": [{"message": {"content": "Cześć! O czym chcesz porozmawiać?"}}],
        }).encode())
        with patch.dict("os.environ", {
            "CACTUS_SI_API_URL": "https://cactus-si.ai",
            "CACTUS_SI_API_KEY": "test-key",
        }), patch("hyperos.web_server.urlopen", return_value=response) as upstream:
            answer = cactus_si_generate([
                {"role": "system", "content": "system"},
                {"role": "user", "content": "Cześć"},
            ])
        request = upstream.call_args.args[0]
        payload = json.loads(request.data)
        self.assertEqual(answer, "Cześć! O czym chcesz porozmawiać?")
        self.assertEqual(request.full_url, "https://cactus-si.ai")
        authorization = request.get_header("Authorization").split()
        self.assertEqual(authorization[0], "Bearer")
        self.assertEqual(authorization[1], "test-key")
        self.assertEqual(payload["model"], CACTUS_SI_MODEL)
        self.assertEqual(payload["messages"][-1], {"role": "user", "content": "Cześć"})
        self.assertEqual(payload["temperature"], 0.7)
        self.assertEqual(payload["calibration_pattern"], "364")

    def test_cactus_si_requires_https_and_both_configuration_values(self):
        with patch.dict("os.environ", {
            "CACTUS_SI_API_URL": "http://cactus-si.ai",
            "CACTUS_SI_API_KEY": "test-key",
        }):
            with self.assertRaisesRegex(ValueError, "HTTPS"):
                cactus_si_generate([{"role": "user", "content": "Cześć"}])
        with patch.dict("os.environ", {
            "CACTUS_SI_API_URL": "https://cactus-si.ai",
            "CACTUS_SI_API_KEY": "",
        }):
            with self.assertRaisesRegex(ValueError, "HTTPS"):
                cactus_si_generate([{"role": "user", "content": "Cześć"}])

    def test_repository_change_prompt_is_json_only_and_uses_large_output_budget(self):
        response_body = json.dumps({"changes": [{
            "path": "src/main.py",
            "action": "modify",
            "replacements": [{"old_text": "before", "new_text": "after"}],
        }]})
        with patch("hyperos.web_server.gemini_api_key", return_value="key"), patch(
                "hyperos.web_server.gemini_generate", return_value=response_body) as generate:
            changes = repo_change_response("Zmień kod", {"src/main.py": "before\n"})
        self.assertEqual(changes[0]["path"], "src/main.py")
        self.assertTrue(generate.call_args.kwargs["json_response"])
        self.assertEqual(generate.call_args.kwargs["max_output_tokens"], 8192)
        messages = generate.call_args.args[0]
        self.assertIn("niezaufanym materiałem", messages[0]["content"])
        self.assertIn('"before\\n"', messages[1]["content"])

        with patch("hyperos.web_server.gemini_api_key", return_value=""), patch(
                "hyperos.web_server.gemini_generate") as generate:
            with self.assertRaisesRegex(RuntimeError, "GEMINI_API_KEY"):
                repo_change_response("Zmień kod", {"src/main.py": "before\n"})
            generate.assert_not_called()

    def test_chat_image_validation_enforces_format_and_limits(self):
        image = "data:image/jpeg;base64," + base64.b64encode(b"\xff\xd8\xfftest").decode()
        messages = validate_chat_messages([{
            "role": "user", "content": "opisz", "images": [image],
        }])
        self.assertEqual(messages[0]["images"], [{
            "mime_type": "image/jpeg", "data": base64.b64encode(b"\xff\xd8\xfftest").decode(),
        }])
        with self.assertRaisesRegex(ValueError, "zakodowane"):
            validate_chat_messages([{
                "role": "user", "content": "opisz", "images": ["data:image/svg+xml;base64,PHN2Zz4="],
            }])
        with self.assertRaisesRegex(ValueError, "maksymalnie 4"):
            validate_chat_messages([{
                "role": "user", "content": "opisz", "images": [image] * 5,
            }])
        too_large = "data:image/jpeg;base64," + base64.b64encode(
            b"\xff\xd8\xff" + b"x" * (1024 * 1024),
        ).decode()
        with self.assertRaisesRegex(ValueError, "1 MiB"):
            validate_chat_messages([{
                "role": "user", "content": "opisz", "images": [too_large],
            }])

    def test_transient_gemini_failure_falls_back_to_ollama(self):
        with patch("hyperos.web_server.gemini_api_key", return_value="key"), patch(
                "hyperos.web_server.propose_with_gemini", side_effect=URLError("offline")), patch(
                "hyperos.web_server.propose_with_ollama",
                return_value={"reply": "fallback", "command": None}) as fallback:
            result = propose_with_primary_model("hello")
        self.assertEqual(result["reply"], "fallback")
        fallback.assert_called_once()

    def test_gemini_auth_failure_does_not_fall_back(self):
        error = HTTPError("https://example.invalid", 401, "unauthorized", {}, None)
        with patch("hyperos.web_server.gemini_api_key", return_value="key"), patch(
                "hyperos.web_server.propose_with_gemini", side_effect=error), patch(
                "hyperos.web_server.propose_with_ollama") as fallback:
            with self.assertRaisesRegex(RuntimeError, "HTTP 401"):
                propose_with_primary_model("hello")
        fallback.assert_not_called()
        error.close()

    def test_dotenv_loads_only_supported_provider_settings(self):
        from pathlib import Path
        from tempfile import TemporaryDirectory

        with TemporaryDirectory() as directory:
            env_file = Path(directory) / ".env"
            env_file.write_text(
                'GEMINI_API_KEY="from-file"\n'
                'CACTUS_SI_API_URL="https://cactus-si.ai"\n'
                'CACTUS_SI_API_KEY="test-key"\n'
                'OTHER_SECRET=ignored\n'
            )
            with patch.dict("os.environ", {}, clear=True):
                load_dotenv(env_file)
                self.assertEqual(os.environ["GEMINI_API_KEY"], "from-file")
                self.assertEqual(os.environ["CACTUS_SI_API_URL"], "https://cactus-si.ai")
                self.assertEqual(os.environ["CACTUS_SI_API_KEY"], "test-key")
                self.assertNotIn("OTHER_SECRET", os.environ)
                os.environ["GEMINI_API_KEY"] = "from-environment"
                load_dotenv(env_file)
                self.assertEqual(os.environ["GEMINI_API_KEY"], "from-file")

    def test_http_requires_origin_host_and_csrf_before_proposing(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), http.client.HTTPConnection)
        port = server.server_port
        handler = make_handler(self.broker, "test-csrf", port)
        server.RequestHandlerClass = handler
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
        headers = {
            "Content-Type": "application/json",
            "Origin": f"http://localhost:{port}",
            "X-Terminal-CSRF": "test-csrf",
        }
        try:
            for changes in ({"Origin": "https://example.com"},
                            {"X-Terminal-CSRF": "wrong"},
                            {"Host": "example.com"}):
                connection.request("POST", "/api/root/propose", json.dumps({"prompt": "uid"}),
                                   headers | changes)
                response = connection.getresponse()
                self.assertEqual(response.status, 403)
                response.read()
            connection.request("POST", "/api/root/propose", json.dumps({"prompt": "wykonaj id -u"}), headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            proposal = json.loads(response.read())
            self.assertEqual(self.executed, [])
            connection.request("POST", "/api/terminal/resolve", json.dumps({
                "proposal_id": proposal["proposal_id"], "approved": True, "command": "id -u",
            }), headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 400)
            response.read()
            connection.request("POST", "/api/root/resolve", json.dumps({
                "proposal_id": proposal["proposal_id"], "approved": True, "command": "id -u",
            }), headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            response.read()
            self.assertEqual(self.executed, ["id -u"])
            connection.request("POST", "/api/terminal/propose",
                               json.dumps({"prompt": "wykonaj id -u"}), headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            user_proposal = json.loads(response.read())
            connection.request("POST", "/api/root/resolve", json.dumps({
                "proposal_id": user_proposal["proposal_id"], "approved": True,
                "command": "id -u",
            }), headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 400)
            response.read()
            connection.request("POST", "/api/terminal/resolve", json.dumps({
                "proposal_id": user_proposal["proposal_id"], "approved": True,
                "command": "id -u",
            }), headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            response.read()
            self.assertEqual(self.executed_user, ["id -u"])
        finally:
            connection.close()
            server.shutdown()
            server.server_close()
            worker.join()

    def test_https_localhost_origin_is_accepted_with_csrf(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), http.client.HTTPConnection)
        port = server.server_port
        server.RequestHandlerClass = make_handler(self.broker, "test-csrf", port)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
        headers = {"Host": f"localhost:{port}", "Origin": f"https://localhost:{port}",
                   "Content-Type": "application/json", "X-Terminal-CSRF": "test-csrf"}
        try:
            with patch("hyperos.web_server.gemini_api_key", return_value=""), patch(
                    "hyperos.web_server.urlopen", return_value=io.BytesIO(
                    b'{"message":{"content":"Czesc"},"done":true}\n')):
                connection.request("POST", "/api/ollama/chat", json.dumps({
                    "messages": [{"role": "user", "content": "czesc"}],
                }), headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertIn(b"Czesc", response.read())
            connection.request("POST", "/api/root/propose", '{"prompt":"wykonaj id -u"}', headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            response.read()
            connection.request("POST", "/api/root/propose", '{"prompt":"wykonaj id -u"}',
                               headers | {"Origin": f"https://other.example:{port}"})
            response = connection.getresponse()
            self.assertEqual(response.status, 403)
            response.read()
            self.assertEqual(self.executed, [])
        finally:
            connection.close()
            server.shutdown()
            server.server_close()
            worker.join()

    def test_web_chat_uses_gemini_and_falls_back_on_transient_failure(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), http.client.HTTPConnection)
        port = server.server_port
        server.RequestHandlerClass = make_handler(self.broker, "test-csrf", port)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
        headers = {"Host": f"localhost:{port}", "Origin": f"http://localhost:{port}",
                   "Content-Type": "application/json", "X-Terminal-CSRF": "test-csrf"}
        payload = json.dumps({"messages": [
            {"role": "system", "content": "system"},
            {"role": "user", "content": "hello"},
        ]})
        try:
            with patch("hyperos.web_server.gemini_api_key", return_value="key"), patch(
                    "hyperos.web_server.gemini_generate", return_value="Gemini reply") as gemini:
                connection.request("POST", "/api/ollama/chat", payload, headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(response.getheader("X-Cactus-Provider"), "Gemini")
                self.assertIn(b"Gemini reply", response.read())

            with patch("hyperos.web_server.gemini_api_key", return_value="key"), patch(
                    "hyperos.web_server.gemini_generate", side_effect=URLError("offline")), patch(
                    "hyperos.web_server.urlopen", return_value=io.BytesIO(
                        b'{"message":{"content":"Ollama fallback"},"done":true}\n')) as upstream:
                connection.request("POST", "/api/ollama/chat", payload, headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(response.getheader("X-Cactus-Provider"), "Ollama fallback")
                self.assertIn(b"Ollama fallback", response.read())
                forwarded = json.loads(upstream.call_args.args[0].data)
                self.assertEqual(forwarded["messages"][1]["images"], [])
        finally:
            connection.close()
            server.shutdown()
            server.server_close()
            worker.join()

    def test_web_chat_uses_cactus_si_for_free_conversation(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), http.client.HTTPConnection)
        port = server.server_port
        server.RequestHandlerClass = make_handler(self.broker, "test-csrf", port)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
        headers = {"Host": f"localhost:{port}", "Origin": f"http://localhost:{port}",
                   "Content-Type": "application/json", "X-Terminal-CSRF": "test-csrf"}
        payload = json.dumps({"messages": [
            {"role": "system", "content": "Rozmawiaj swobodnie."},
            {"role": "user", "content": "Cześć, porozmawiajmy o muzyce."},
        ]})
        try:
            with patch.dict("os.environ", {
                "CACTUS_SI_API_URL": "https://cactus-si.ai",
                "CACTUS_SI_API_KEY": "test-key",
                "GEMINI_API_KEY": "",
            }), patch("hyperos.web_server.urlopen", return_value=io.BytesIO(json.dumps({
                "choices": [{"message": {"content": "Jasne, jakiej muzyki lubisz słuchać?"}}],
            }).encode())) as upstream:
                connection.request("POST", "/api/ollama/chat", payload, headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(response.getheader("X-Cactus-Provider"), "Cactus SI")
                self.assertIn("jakiej muzyki", response.read().decode())
                self.assertEqual(
                    upstream.call_args.args[0].full_url,
                    "https://cactus-si.ai",
                )
        finally:
            connection.close()
            server.shutdown()
            server.server_close()
            worker.join()

    def test_chat_images_always_use_local_llama_vision(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), http.client.HTTPConnection)
        port = server.server_port
        server.RequestHandlerClass = make_handler(self.broker, "test-csrf", port)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
        headers = {"Host": f"localhost:{port}", "Origin": f"http://localhost:{port}",
                   "Content-Type": "application/json", "X-Terminal-CSRF": "test-csrf"}
        encoded_image = base64.b64encode(b"\xff\xd8\xfftest").decode()
        payload = json.dumps({"messages": [
            {"role": "system", "content": "system"},
            {"role": "user", "content": "opisz obraz", "images": [
                f"data:image/jpeg;base64,{encoded_image}",
            ]},
        ]})
        try:
            with patch("hyperos.web_server.gemini_api_key", return_value="key"), patch(
                    "hyperos.web_server.gemini_generate", return_value="not used") as gemini, patch(
                    "hyperos.web_server.urlopen", return_value=io.BytesIO(
                        b'{"message":{"content":"Llama reply"},"done":true}\n')) as upstream:
                connection.request("POST", "/api/ollama/chat", payload, headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(response.getheader("X-Cactus-Provider"), "Ollama · Llama Vision")
                self.assertIn(b"Llama reply", response.read())
                gemini.assert_not_called()
                forwarded = json.loads(upstream.call_args.args[0].data)
                self.assertEqual(forwarded["model"], "cactus-ai")
                self.assertEqual(forwarded["messages"][1]["images"], [encoded_image])
        finally:
            connection.close()
            server.shutdown()
            server.server_close()
            worker.join()

    def test_repository_changes_require_diff_approval_and_write_only_in_workspace(self):
        from pathlib import Path
        from tempfile import TemporaryDirectory

        with TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            (root / "src/main.py").write_text("value = 1\n", encoding="utf-8")
            (root / ".env").write_text("SECRET=hidden\n", encoding="utf-8")
            server = ThreadingHTTPServer(("127.0.0.1", 0), http.client.HTTPConnection)
            port = server.server_port
            server.RequestHandlerClass = make_handler(
                self.broker, "test-csrf", port,
                repo_broker=RepositoryChangeBroker(root),
            )
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
            headers = {
                "Host": f"localhost:{port}",
                "Origin": f"http://localhost:{port}",
                "Content-Type": "application/json",
                "X-Terminal-CSRF": "test-csrf",
            }
            try:
                connection.request("GET", "/api/repo/files", headers={"Host": headers["Host"]})
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(json.loads(response.read())["files"], ["src/main.py"])

                payload = json.dumps({
                    "mission": "Zmień wartość",
                    "paths": ["src/main.py"],
                })
                with patch("hyperos.web_server.repo_change_response", return_value=[{
                    "path": "src/main.py",
                    "action": "modify",
                    "replacements": [{"old_text": "value = 1", "new_text": "value = 2"}],
                }]) as generate:
                    connection.request("POST", "/api/repo/propose", payload, headers)
                    response = connection.getresponse()
                    self.assertEqual(response.status, 200)
                    proposal = json.loads(response.read())
                    generate.assert_called_once()
                    self.assertEqual(generate.call_args.args[1], {"src/main.py": "value = 1\n"})
                self.assertIn("+value = 2", proposal["changes"][0]["diff"])
                self.assertEqual((root / "src/main.py").read_text(), "value = 1\n")

                connection.request("GET", "/api/repo/pending", headers={"Host": headers["Host"]})
                pending_response = connection.getresponse()
                self.assertEqual(pending_response.status, 200)
                pending = json.loads(pending_response.read())["proposals"]
                self.assertEqual(len(pending), 1)
                self.assertEqual(pending[0]["proposal_id"], proposal["proposal_id"])
                self.assertEqual(pending[0]["changes"], proposal["changes"])

                connection.request("POST", "/api/repo/resolve", json.dumps({
                    "proposal_id": proposal["proposal_id"], "approved": True,
                }), headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(json.loads(response.read())["applied"], ["src/main.py"])
                self.assertEqual((root / "src/main.py").read_text(), "value = 2\n")
                connection.request("GET", "/api/repo/pending", headers={"Host": headers["Host"]})
                pending_response = connection.getresponse()
                self.assertEqual(json.loads(pending_response.read())["proposals"], [])

                connection.request("POST", "/api/repo/propose", payload,
                                   headers | {"X-Terminal-CSRF": "wrong"})
                response = connection.getresponse()
                self.assertEqual(response.status, 403)
                response.read()
            finally:
                connection.close()
                server.shutdown()
                server.server_close()
                worker.join()

    def test_forwarded_chat_and_explicit_root_approval(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), http.client.HTTPConnection)
        port = server.server_port
        origin = "https://test-space-4180.app.github.dev"
        server.RequestHandlerClass = make_handler(self.broker, "test-csrf", port, origin)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
        headers = {"Host": "test-space-4180.app.github.dev", "Origin": origin,
                   "Content-Type": "application/json", "X-Terminal-CSRF": "test-csrf"}
        payload = json.dumps({"messages": [{"role": "user", "content": "ktora godzina?"}]})
        try:
            with patch("hyperos.web_server.gemini_api_key", return_value=""), patch(
                    "hyperos.web_server.urlopen", return_value=io.BytesIO(
                    b'{"message":{"content":"20:00 UTC"},"done":true}\n')) as upstream:
                connection.request("POST", "/api/ollama/chat", payload, headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertIn(b"20:00 UTC", response.read())
                self.assertEqual(upstream.call_args.args[0].full_url,
                                 "http://127.0.0.1:11434/api/chat")
            connection.request("POST", "/api/root/propose", '{"prompt":"wykonaj id -u"}', headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            proposal = json.loads(response.read())
            self.assertEqual(self.executed, [])
            for changes in ({"Origin": "https://other.example"},
                            {"Host": "other.example"},
                            {"X-Terminal-CSRF": "wrong"}):
                connection.request("POST", "/api/root/resolve", json.dumps({
                    "proposal_id": proposal["proposal_id"], "approved": True, "command": "id -u",
                }), headers | changes)
                response = connection.getresponse()
                self.assertEqual(response.status, 403)
                response.read()
                self.assertEqual(self.executed, [])
            connection.request("POST", "/api/root/resolve", json.dumps({
                "proposal_id": proposal["proposal_id"], "approved": True, "command": "id -u",
            }), headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            response.read()
            self.assertEqual(self.executed, ["id -u"])
            connection.request("POST", "/api/root/propose", '{"prompt":"wykonaj id -u"}', headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            proposal = json.loads(response.read())
            connection.request("POST", "/api/root/resolve", json.dumps({
                "proposal_id": proposal["proposal_id"], "approved": False, "command": "id -u",
            }), headers)
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            self.assertTrue(json.loads(response.read())["cancelled"])
            connection.request("GET", "/api/root/status", headers={"Host": headers["Host"]})
            with_status = connection.getresponse()
            self.assertEqual(with_status.status, 200)
            self.assertTrue(json.loads(with_status.read())["execution_allowed"])
            with patch("hyperos.web_server.urlopen", side_effect=URLError("offline")):
                connection.request("GET", "/api/ollama/version", headers={"Host": headers["Host"]})
                response = connection.getresponse()
                self.assertEqual(response.status, 503)
                response.read()
            self.assertEqual(self.executed, ["id -u"])
        finally:
            connection.close()
            server.shutdown()
            server.server_close()
            worker.join()


if __name__ == "__main__":
    unittest.main()