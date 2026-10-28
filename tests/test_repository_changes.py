from pathlib import Path
import tempfile
import unittest

from hyperos.repository_changes import (
    MAX_DOWNLOAD_BYTES,
    RepositoryChangeBroker,
    RepositoryConflictError,
)


class RepositoryChangeBrokerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "src").mkdir()
        (self.root / "src/main.py").write_text("value = 1\n", encoding="utf-8")
        (self.root / ".env").write_text("SECRET=hidden\n", encoding="utf-8")
        (self.root / ".git").mkdir()
        (self.root / ".git/config").write_text("hidden\n", encoding="utf-8")
        self.broker = RepositoryChangeBroker(self.root)

    def test_file_listing_excludes_hidden_secrets_and_git(self):
        self.assertEqual(self.broker.list_files(), ["src/main.py"])
        with self.assertRaisesRegex(ValueError, "Ścieżka poza"):
            self.broker.read_files(["../.env"])
        with self.assertRaisesRegex(ValueError, "Ścieżka poza"):
            self.broker.read_files([".env"])

    def test_download_listing_includes_bounded_binary_files_and_reads_bytes(self):
        assets = self.root / "assets"
        assets.mkdir()
        attachment = assets / "picture.bin"
        attachment.write_bytes(b"\x00\x01attachment\xff")
        too_large = assets / "too-large.bin"
        with too_large.open("wb") as stream:
            stream.seek(MAX_DOWNLOAD_BYTES)
            stream.write(b"x")

        self.assertEqual(self.broker.read_download("assets/picture.bin"), attachment.read_bytes())
        self.assertEqual(
            self.broker.list_downloadable_files(),
            [
                {"path": "assets/picture.bin", "size": attachment.stat().st_size},
                {"path": "src/main.py", "size": (self.root / "src/main.py").stat().st_size},
            ],
        )
        with self.assertRaises(ValueError):
            self.broker.read_download("assets/too-large.bin")
        with self.assertRaises(ValueError):
            self.broker.read_download(".env")
        with self.assertRaises(ValueError):
            self.broker.read_download("../outside.bin")

    def test_changes_are_reviewed_before_write_and_approval_applies_atomically(self):
        context = self.broker.read_files(["src/main.py"])
        proposal = self.broker.propose_changes(context, [
            {
                "path": "src/main.py",
                "action": "modify",
                "replacements": [{"old_text": "value = 1", "new_text": "value = 2"}],
            },
            {"path": "src/new.py", "action": "create", "content": "print('new')\n"},
        ])
        self.assertIn("-value = 1", proposal["changes"][0]["diff"])
        self.assertIn("+value = 2", proposal["changes"][0]["diff"])
        self.assertEqual((self.root / "src/main.py").read_text(), "value = 1\n")
        self.assertFalse((self.root / "src/new.py").exists())

        result = self.broker.resolve(proposal["proposal_id"], True)
        self.assertEqual(result["applied"], ["src/main.py", "src/new.py"])
        self.assertEqual((self.root / "src/main.py").read_text(), "value = 2\n")
        self.assertEqual((self.root / "src/new.py").read_text(), "print('new')\n")
        with self.assertRaisesRegex(ValueError, "wykorzystana"):
            self.broker.resolve(proposal["proposal_id"], True)

    def test_pending_queue_lists_diffs_and_drops_expired_proposals(self):
        now = [100.0]
        broker = RepositoryChangeBroker(self.root, clock=lambda: now[0])
        proposal = broker.propose_changes(broker.read_files(["src/main.py"]), [{
            "path": "src/new.py", "action": "create", "content": "print('new')\n",
        }])

        pending = broker.list_pending()
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0]["proposal_id"], proposal["proposal_id"])
        self.assertEqual(pending[0]["changes"], proposal["changes"])
        self.assertGreater(pending[0]["expires_in"], 0)

        now[0] += 601
        self.assertEqual(broker.list_pending(), [])
        self.assertEqual(broker.pending, {})

    def test_rejection_and_stale_file_never_write_proposed_content(self):
        original = self.broker.read_files(["src/main.py"])
        proposal = self.broker.propose_changes(original, [{
            "path": "src/main.py",
            "action": "modify",
            "replacements": [{"old_text": "value = 1", "new_text": "value = 2"}],
        }])
        self.assertEqual(self.broker.resolve(proposal["proposal_id"], False), {"cancelled": True})
        self.assertEqual((self.root / "src/main.py").read_text(), "value = 1\n")

        proposal = self.broker.propose_changes(original, [{
            "path": "src/main.py",
            "action": "modify",
            "replacements": [{"old_text": "value = 1", "new_text": "value = 3"}],
        }])
        (self.root / "src/main.py").write_text("operator edit\n", encoding="utf-8")
        with self.assertRaises(RepositoryConflictError):
            self.broker.resolve(proposal["proposal_id"], True)
        self.assertEqual((self.root / "src/main.py").read_text(), "operator edit\n")

    def test_rejects_unselected_files_path_traversal_and_ambiguous_replacements(self):
        context = self.broker.read_files(["src/main.py"])
        for change in (
            {"path": "src/other.py", "action": "modify",
             "replacements": [{"old_text": "value = 1", "new_text": "value = 2"}]},
            {"path": "../outside.py", "action": "create", "content": "x"},
        ):
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.broker.propose_changes(context, [change])
        repeated_context = {"src/main.py": "value = 1\nvalue = 2\n"}
        with self.assertRaisesRegex(ValueError, "dokładnie raz"):
            self.broker.propose_changes(repeated_context, [{
                "path": "src/main.py",
                "action": "modify",
                "replacements": [{"old_text": "value", "new_text": "changed"}],
            }])

    def test_refuses_symlinked_paths_and_existing_create_targets(self):
        outside = self.root.parent / "outside.txt"
        outside.write_text("outside\n", encoding="utf-8")
        link = self.root / "src/outside.txt"
        link.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "dowiązania"):
            self.broker.read_files(["src/outside.txt"])
        with self.assertRaises(ValueError):
            self.broker.read_download("src/outside.txt")
        (self.root / "src/other.py").write_text("other\n", encoding="utf-8")
        with self.assertRaises(RepositoryConflictError):
            self.broker.propose_changes(self.broker.read_files(["src/other.py"]), [{
                "path": "src/main.py", "action": "create", "content": "overwritten",
            }])
        self.assertEqual(outside.read_text(), "outside\n")


if __name__ == "__main__":
    unittest.main()
