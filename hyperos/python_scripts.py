"""Explicit, single-use approvals for running workspace Python scripts."""

import hashlib
import secrets
import threading
import time

MAX_PYTHON_SCRIPT_BYTES = 64 * 1024
SCRIPT_PROPOSAL_TTL_SECONDS = 180
MAX_PENDING_SCRIPTS = 8


class PythonScriptBroker:
    def __init__(self, repository, executor, clock=time.monotonic, execution_lock=None):
        self.repository = repository
        self.executor = executor
        self.clock = clock
        self.pending = {}
        self.lock = threading.Lock()
        self.execution_lock = execution_lock or threading.Lock()

    def list_scripts(self):
        return [path for path in self.repository.list_files()
                if path.lower().endswith(".py")]

    def propose(self, path):
        if not isinstance(path, str) or not path.lower().endswith(".py"):
            raise ValueError("Wybierz plik Python z repozytorium")
        source = self.repository.read_files([path])[path]
        return self._propose(path, source)

    def propose_source(self, source):
        if not isinstance(source, str) or not source.strip():
            raise ValueError("Wklej niepusty skrypt Python")
        return self._propose("Wklejony skrypt", source)

    def _propose(self, path, source):
        if "\x00" in source:
            raise ValueError("Skrypt musi być tekstem UTF-8 bez bajtów NUL")
        if len(source.encode("utf-8")) > MAX_PYTHON_SCRIPT_BYTES:
            raise ValueError("Skrypt przekracza limit 64 KiB")

        digest = hashlib.sha256(source.encode("utf-8")).hexdigest()
        proposal_id = secrets.token_urlsafe(24)
        now = self.clock()
        with self.lock:
            self.pending = {
                key: value for key, value in self.pending.items()
                if value[3] > now
            }
            if len(self.pending) >= MAX_PENDING_SCRIPTS:
                raise ValueError("Za dużo oczekujących propozycji skryptów")
            expires_at = now + SCRIPT_PROPOSAL_TTL_SECONDS
            self.pending[proposal_id] = (path, source, digest, expires_at)

        return {
            "proposal_id": proposal_id,
            "path": path,
            "source": source,
            "sha256": digest,
            "expires_in": SCRIPT_PROPOSAL_TTL_SECONDS,
        }

    def resolve(self, proposal_id, approved, displayed_sha256):
        if (not isinstance(proposal_id, str) or type(approved) is not bool
                or not isinstance(displayed_sha256, str)):
            raise ValueError("Nieprawidłowe zatwierdzenie skryptu")
        with self.lock:
            proposal = self.pending.pop(proposal_id, None)
        if proposal is None or proposal[3] <= self.clock():
            raise ValueError("Propozycja skryptu wygasła lub została już wykorzystana")
        if not approved:
            return {"cancelled": True}
        if not secrets.compare_digest(displayed_sha256, proposal[2]):
            raise ValueError("Podgląd skryptu zmienił się od przygotowania zgody")
        if not self.execution_lock.acquire(blocking=False):
            raise ValueError("Inny skrypt jest już wykonywany")
        try:
            script_path = None if proposal[0] == "Wklejony skrypt" else proposal[0]
            return self.executor(proposal[1], script_path)
        finally:
            self.execution_lock.release()