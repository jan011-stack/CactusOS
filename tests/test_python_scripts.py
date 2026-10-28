import http.client
import json
from pathlib import Path
import threading
import tempfile
import unittest
from http.server import ThreadingHTTPServer
from unittest.mock import patch

from hyperos.python_scripts import PythonScriptBroker
from hyperos.repository_changes import RepositoryChangeBroker
from hyperos.web_server import TerminalBroker, execute_python_source, make_handler


class PythonScriptBrokerTests(unittest.TestCase):
    def test_executor_uses_host_interpreter_and_sudo_without_isolated_mode(self):
        source = "print('approved')\n"
        with patch("hyperos.web_server.os.geteuid", return_value=1000), patch(
                "hyperos.web_server.sys.executable", "/usr/bin/python3"), patch(
                "hyperos.web_server._collect_process_output",
                return_value={"exit_code": 0}) as collector:
            result = execute_python_source(source, "scripts/setup.py")

        arguments = collector.call_args.args[0]
        self.assertEqual(arguments[:9], [
            "sudo", "-n", "--", "/usr/bin/timeout", "-k", "5s", "60s",
            "/usr/bin/python3", "-c",
        ])
        self.assertIn("sys.path[0]", arguments[9])
        self.assertIn("__file__", arguments[9])
        self.assertEqual(arguments[-2:], ["scripts/setup.py", source])
        self.assertNotIn("-I", arguments)
        self.assertEqual(result, {"exit_code": 0})

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "scripts").mkdir()
        (self.root / "scripts/setup.py").write_text("value = 'snapshot'\n", encoding="utf-8")
        (self.root / "scripts/setup.sh").write_text("echo ignored\n", encoding="utf-8")
        self.executed = []
        repository = RepositoryChangeBroker(self.root)
        self.broker = PythonScriptBroker(
            repository,
            lambda source, script_path: self.executed.append((source, script_path)) or {"exit_code": 0},
        )

    def test_lists_python_files_and_requires_explicit_approval(self):
        self.assertEqual(self.broker.list_scripts(), ["scripts/setup.py"])
        proposal = self.broker.propose("scripts/setup.py")
        self.assertEqual(proposal["source"], "value = 'snapshot'\n")
        self.assertEqual(self.executed, [])

        result = self.broker.resolve(proposal["proposal_id"], False, proposal["sha256"])
        self.assertEqual(result, {"cancelled": True})
        self.assertEqual(self.executed, [])

    def test_approval_runs_the_exact_preview_snapshot_once(self):
        proposal = self.broker.propose("scripts/setup.py")
        (self.root / "scripts/setup.py").write_text("value = 'changed'\n", encoding="utf-8")

        result = self.broker.resolve(proposal["proposal_id"], True, proposal["sha256"])
        self.assertEqual(result, {"exit_code": 0})
        self.assertEqual(self.executed, [("value = 'snapshot'\n", "scripts/setup.py")])
        with self.assertRaisesRegex(ValueError, "wykorzystana"):
            self.broker.resolve(proposal["proposal_id"], True, proposal["sha256"])

    def test_rejects_digest_tampering_and_non_python_paths(self):
        proposal = self.broker.propose("scripts/setup.py")
        with self.assertRaisesRegex(ValueError, "Podgląd skryptu"):
            self.broker.resolve(proposal["proposal_id"], True, "0" * 64)
        self.assertEqual(self.executed, [])
        for path in ("scripts/setup.sh", "../outside.py"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                self.broker.propose(path)

    def test_pasted_script_requires_preview_and_approval(self):
        source = "print('pasted')\n"
        proposal = self.broker.propose_source(source)
        self.assertEqual(proposal["path"], "Wklejony skrypt")
        self.assertEqual(proposal["source"], source)
        self.assertEqual(self.executed, [])

        result = self.broker.resolve(proposal["proposal_id"], True, proposal["sha256"])
        self.assertEqual(result, {"exit_code": 0})
        self.assertEqual(self.executed, [(source, None)])
        for invalid_source in ("", "   ", "x\x00y", "x" * (64 * 1024 + 1)):
            with self.subTest(source=invalid_source[:12]), self.assertRaises(ValueError):
                self.broker.propose_source(invalid_source)

    def test_http_script_flow_lists_previews_and_runs_only_after_approval(self):
        repository = RepositoryChangeBroker(self.root)
        executed = []
        terminal = TerminalBroker(executor=lambda command: {"exit_code": 0})
        scripts = PythonScriptBroker(
            repository,
            lambda source, script_path: executed.append((source, script_path))
            or {"exit_code": 0, "output": "done"},
            execution_lock=terminal.execution_lock,
        )
        server = ThreadingHTTPServer(("127.0.0.1", 0), http.client.HTTPConnection)
        port = server.server_port
        server.RequestHandlerClass = make_handler(
            terminal, "test-csrf", port, repo_broker=repository, script_broker=scripts,
        )
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
        headers = {
            "Content-Type": "application/json",
            "Origin": f"http://localhost:{port}",
            "X-Terminal-CSRF": "test-csrf",
        }
        try:
            connection.request("GET", "/api/root/scripts")
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            self.assertEqual(json.loads(response.read())["scripts"], ["scripts/setup.py"])

            with patch("hyperos.web_server.root_available", return_value=True):
                connection.request("POST", "/api/root/script/propose",
                                   json.dumps({"path": "scripts/setup.py"}), headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                proposal = json.loads(response.read())
            self.assertEqual(executed, [])

            with patch("hyperos.web_server.root_available", return_value=True):
                connection.request("POST", "/api/root/script/resolve", json.dumps({
                    "proposal_id": proposal["proposal_id"],
                    "approved": True,
                    "sha256": proposal["sha256"],
                }), headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(json.loads(response.read())["exit_code"], 0)
            self.assertEqual(executed, [("value = 'snapshot'\n", "scripts/setup.py")])

            pasted_source = "print('pasted')\n"
            with patch("hyperos.web_server.root_available", return_value=True):
                connection.request("POST", "/api/root/script/propose",
                                   json.dumps({"source": pasted_source}), headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                pasted_proposal = json.loads(response.read())
            self.assertEqual(pasted_proposal["source"], pasted_source)
            with patch("hyperos.web_server.root_available", return_value=True):
                connection.request("POST", "/api/root/script/resolve", json.dumps({
                    "proposal_id": pasted_proposal["proposal_id"],
                    "approved": True,
                    "sha256": pasted_proposal["sha256"],
                }), headers)
                response = connection.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(json.loads(response.read())["exit_code"], 0)
            self.assertEqual(executed, [
                ("value = 'snapshot'\n", "scripts/setup.py"),
                (pasted_source, None),
            ])
        finally:
            connection.close()
            server.shutdown()
            worker.join(timeout=5)
            server.server_close()


if __name__ == "__main__":
    unittest.main()