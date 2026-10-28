"""Workspace-bounded file proposals requiring explicit operator approval."""

import difflib
import os
from pathlib import Path, PurePosixPath
import secrets
import stat
import tempfile
import threading
import time

MAX_SELECTED_FILES = 8
MAX_FILE_BYTES = 128 * 1024
MAX_DOWNLOAD_BYTES = 20 * 1024 * 1024
MAX_CONTEXT_BYTES = 200 * 1024
MAX_CHANGES = 8
MAX_PROPOSAL_BYTES = 256 * 1024
PROPOSAL_TTL_SECONDS = 600
IGNORED_DIRECTORIES = {
    ".git", "__pycache__", "node_modules", "target", "dist", "build", ".venv", "venv",
}
IGNORED_SUFFIXES = {".pem", ".key", ".p12", ".pfx", ".crt", ".cer"}


class RepositoryConflictError(RuntimeError):
    """A proposed change no longer matches the current workspace state."""


def _validate_path(path):
    if (not isinstance(path, str) or not path or len(path) > 240
            or "\\" in path or "\x00" in path):
        raise ValueError("Nieprawidłowa ścieżka pliku")
    parsed = PurePosixPath(path)
    if (parsed.is_absolute() or str(parsed) != path
            or any(part in {"", ".", ".."} or part.startswith(".")
                   or part in IGNORED_DIRECTORIES for part in parsed.parts)):
        raise ValueError("Ścieżka poza dozwolonym zakresem repozytorium")
    if parsed.name.lower() == ".env" or parsed.suffix.lower() in IGNORED_SUFFIXES:
        raise ValueError("Pliki sekretów i kluczy nie są dostępne agentom")
    return parsed


class RepositoryChangeBroker:
    def __init__(self, root, clock=time.monotonic):
        self.root = Path(root).resolve()
        self.clock = clock
        self.pending = {}
        self.lock = threading.Lock()

    def _checked_path(self, relative_path, *, allow_missing=False):
        parsed = _validate_path(relative_path)
        candidate = self.root
        for index, part in enumerate(parsed.parts):
            candidate = candidate / part
            try:
                details = candidate.lstat()
            except FileNotFoundError:
                if allow_missing and index == len(parsed.parts) - 1:
                    break
                raise ValueError("Plik lub katalog repozytorium nie istnieje")
            if stat.S_ISLNK(details.st_mode):
                raise ValueError("Ścieżki przez dowiązania symboliczne są niedozwolone")
            if index < len(parsed.parts) - 1 and not stat.S_ISDIR(details.st_mode):
                raise ValueError("Element ścieżki nie jest katalogiem")
        try:
            candidate.resolve(strict=False).relative_to(self.root)
        except ValueError as error:
            raise ValueError("Ścieżka poza repozytorium") from error
        return candidate

    def list_files(self):
        return self._list_files(MAX_FILE_BYTES)

    def list_downloadable_files(self):
        return self._list_files(MAX_DOWNLOAD_BYTES, include_size=True)

    def _list_files(self, max_file_bytes, *, include_size=False):
        files = []
        for directory, names, filenames in os.walk(self.root, followlinks=False):
            base = Path(directory)
            names[:] = [
                name for name in names
                if name not in IGNORED_DIRECTORIES and not name.startswith(".")
                and not (base / name).is_symlink()
            ]
            for name in filenames:
                path = base / name
                relative = path.relative_to(self.root).as_posix()
                try:
                    _validate_path(relative)
                    if path.is_symlink() or not path.is_file():
                        continue
                    size = path.stat().st_size
                    if size > max_file_bytes:
                        continue
                except (OSError, ValueError):
                    continue
                files.append({"path": relative, "size": size} if include_size else relative)
                if len(files) >= 1000:
                    return sorted(files, key=lambda item: item["path"] if include_size else item)
        return sorted(files, key=lambda item: item["path"] if include_size else item)

    def read_download(self, relative_path):
        parsed = _validate_path(relative_path)
        directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
        file_flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
        root_fd = os.open(self.root, directory_flags)
        current_fd = root_fd
        file_fd = None
        try:
            for part in parsed.parts[:-1]:
                next_fd = os.open(part, directory_flags, dir_fd=current_fd)
                if current_fd != root_fd:
                    os.close(current_fd)
                current_fd = next_fd
            file_fd = os.open(parsed.parts[-1], file_flags, dir_fd=current_fd)
            details = os.fstat(file_fd)
            if not stat.S_ISREG(details.st_mode):
                raise ValueError("Można pobierać tylko zwykłe pliki")
            if details.st_size > MAX_DOWNLOAD_BYTES:
                raise ValueError("Plik przekracza limit pobierania 20 MiB")
            with os.fdopen(file_fd, "rb") as stream:
                file_fd = None
                content = stream.read(MAX_DOWNLOAD_BYTES + 1)
            if len(content) > MAX_DOWNLOAD_BYTES:
                raise ValueError("Plik przekracza limit pobierania 20 MiB")
            return content
        except OSError as error:
            raise ValueError("Plik lub katalog repozytorium nie istnieje albo jest niedostępny") from error
        finally:
            if file_fd is not None:
                os.close(file_fd)
            if current_fd != root_fd:
                os.close(current_fd)
            os.close(root_fd)

    def read_files(self, paths):
        if (not isinstance(paths, list) or not 1 <= len(paths) <= MAX_SELECTED_FILES
                or any(not isinstance(path, str) for path in paths)
                or len(set(paths)) != len(paths)):
            raise ValueError(f"Wybierz od 1 do {MAX_SELECTED_FILES} różnych plików")
        result = {}
        total_bytes = 0
        for relative in paths:
            path = self._checked_path(relative)
            try:
                details = path.stat()
                if not stat.S_ISREG(details.st_mode) or details.st_size > MAX_FILE_BYTES:
                    raise ValueError("Plik nie jest zwykłym plikiem lub przekracza limit 128 KiB")
                content = path.read_text(encoding="utf-8")
            except UnicodeDecodeError as error:
                raise ValueError("Wybrany plik nie jest tekstem UTF-8") from error
            if "\x00" in content:
                raise ValueError("Pliki binarne nie są dostępne agentom")
            total_bytes += len(content.encode("utf-8"))
            if total_bytes > MAX_CONTEXT_BYTES:
                raise ValueError("Łączny rozmiar wybranych plików przekracza limit 200 KiB")
            result[relative] = content
        return result

    def propose_changes(self, original_files, changes):
        if not isinstance(original_files, dict) or not original_files:
            raise ValueError("Brak kontekstu plików dla propozycji")
        if not isinstance(changes, list) or not 1 <= len(changes) <= MAX_CHANGES:
            raise ValueError(f"Agent może zaproponować od 1 do {MAX_CHANGES} zmian")

        proposed_files = []
        seen_paths = set()
        total_bytes = 0
        for change in changes:
            if not isinstance(change, dict):
                raise ValueError("Nieprawidłowy format zmiany")
            relative = change.get("path")
            _validate_path(relative)
            if relative in seen_paths:
                raise ValueError("Propozycja zawiera powtórzoną ścieżkę")
            seen_paths.add(relative)
            action = change.get("action")
            if action == "modify":
                if relative not in original_files:
                    raise ValueError("Agent może modyfikować tylko wybrane pliki")
                original = original_files[relative]
                content = original
                replacements = change.get("replacements")
                if not isinstance(replacements, list) or not replacements:
                    raise ValueError("Zmiana pliku wymaga co najmniej jednej poprawki")
                for replacement in replacements:
                    if (not isinstance(replacement, dict)
                            or not isinstance(replacement.get("old_text"), str)
                            or not replacement["old_text"]
                            or not isinstance(replacement.get("new_text"), str)):
                        raise ValueError("Nieprawidłowa poprawka tekstu")
                    old_text = replacement["old_text"]
                    if content.count(old_text) != 1:
                        raise ValueError("Fragment do zmiany nie występuje dokładnie raz w pliku")
                    content = content.replace(old_text, replacement["new_text"], 1)
            elif action == "create":
                if relative in original_files:
                    raise ValueError("Nie można utworzyć pliku, który był już wybrany")
                content = change.get("content")
                if not isinstance(content, str):
                    raise ValueError("Treść nowego pliku musi być tekstem")
                original = None
            else:
                raise ValueError("Dozwolone są wyłącznie tworzenie i modyfikowanie plików")

            encoded = content.encode("utf-8")
            if "\x00" in content:
                raise ValueError("Proponowane pliki muszą być tekstem UTF-8")
            total_bytes += len(encoded)
            if len(encoded) > MAX_FILE_BYTES or total_bytes > MAX_PROPOSAL_BYTES:
                raise ValueError("Proponowane zmiany przekraczają limit rozmiaru")
            target = self._checked_path(relative, allow_missing=action == "create")
            if action == "create" and target.exists():
                raise RepositoryConflictError("Plik do utworzenia już istnieje")
            if action == "modify":
                current = self.read_files([relative])[relative]
                if current != original:
                    raise RepositoryConflictError("Plik zmienił się podczas przygotowywania propozycji")
            label = f"a/{relative}"
            diff = "".join(difflib.unified_diff(
                (original or "").splitlines(keepends=True),
                content.splitlines(keepends=True),
                fromfile=label if original is not None else "/dev/null",
                tofile=f"b/{relative}",
            ))
            if not diff:
                raise ValueError("Propozycja nie zawiera żadnych zmian")
            proposed_files.append({
                "path": relative,
                "content": content,
                "original": original,
                "diff": diff,
            })

        proposal_id = secrets.token_urlsafe(24)
        expires_at = self.clock() + PROPOSAL_TTL_SECONDS
        with self.lock:
            self.pending[proposal_id] = (proposed_files, expires_at)
        return {
            "proposal_id": proposal_id,
            "expires_in": PROPOSAL_TTL_SECONDS,
            "changes": [{"path": item["path"], "diff": item["diff"]} for item in proposed_files],
        }

    def list_pending(self):
        now = self.clock()
        with self.lock:
            self.pending = {
                proposal_id: proposal
                for proposal_id, proposal in self.pending.items()
                if proposal[1] > now
            }
            return [{
                "proposal_id": proposal_id,
                "expires_in": max(0, int(expires_at - now)),
                "changes": [
                    {"path": item["path"], "diff": item["diff"]}
                    for item in files
                ],
            } for proposal_id, (files, expires_at) in self.pending.items()]

    def resolve(self, proposal_id, approved):
        if not isinstance(proposal_id, str) or type(approved) is not bool:
            raise ValueError("Nieprawidłowe zatwierdzenie propozycji")
        with self.lock:
            proposal = self.pending.pop(proposal_id, None)
        if proposal is None or proposal[1] <= self.clock():
            raise ValueError("Propozycja wygasła lub została już wykorzystana")
        if not approved:
            return {"cancelled": True}

        files = proposal[0]
        with self.lock:
            for item in files:
                path = self._checked_path(item["path"], allow_missing=item["original"] is None)
                if item["original"] is None:
                    if path.exists():
                        raise RepositoryConflictError(f"Plik {item['path']} już istnieje")
                else:
                    try:
                        if not stat.S_ISREG(path.stat().st_mode):
                            raise RepositoryConflictError(f"Plik {item['path']} nie jest zwykłym plikiem")
                        current = path.read_text(encoding="utf-8")
                    except (FileNotFoundError, UnicodeDecodeError, OSError) as error:
                        raise RepositoryConflictError(f"Plik {item['path']} już nie jest dostępny") from error
                    if current != item["original"]:
                        raise RepositoryConflictError(f"Plik {item['path']} zmienił się od przygotowania diffu")

            staged = []
            try:
                for item in files:
                    path = self._checked_path(item["path"], allow_missing=item["original"] is None)
                    mode = stat.S_IMODE(path.stat().st_mode) if item["original"] is not None else 0o644
                    temporary_path = None
                    try:
                        with tempfile.NamedTemporaryFile(
                                mode="w", encoding="utf-8", dir=path.parent,
                                prefix=".cactus-proposal-", delete=False) as temporary:
                            temporary_path = Path(temporary.name)
                            temporary.write(item["content"])
                            temporary.flush()
                            os.fsync(temporary.fileno())
                            os.chmod(temporary.name, mode)
                        staged.append((temporary_path, path))
                    except OSError:
                        if temporary_path is not None:
                            temporary_path.unlink(missing_ok=True)
                        raise
                for temporary, path in staged:
                    os.replace(temporary, path)
            finally:
                for temporary, _ in staged:
                    try:
                        temporary.unlink(missing_ok=True)
                    except OSError:
                        pass
        return {"applied": [item["path"] for item in files]}
