"""Local file transfer for CactusOS terminal.

Allows uploading files to a workspace directory so the system
can read them and generate/process content.
Loopback-only. No external exposure.
"""

from __future__ import annotations

import base64
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Dict, Optional
from urllib.parse import parse_qs, urlparse

UPLOAD_DIR = Path(os.environ.get("CACTUS_UPLOAD_DIR", "./uploads"))
UPLOAD_DIR.mkdir(parents=True, exist_ok=True)
MAX_BYTES = 20 * 1024 * 1024  # 20 MB


class FileTransferHandler(BaseHTTPRequestHandler):
    def _json(self, code: int, data: Dict[str, Any]) -> None:
        body = json.dumps(data, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        if parsed.path == "/files":
            files = []
            for p in sorted(UPLOAD_DIR.iterdir()):
                if p.is_file():
                    files.append({"name": p.name, "size": p.stat().st_size})
            self._json(200, {"ok": True, "files": files, "dir": str(UPLOAD_DIR)})
            return
        if parsed.path == "/":
            html = """<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>CactusOS File Transfer</title>
<style>body{font-family:monospace;background:#111;color:#eee;padding:2em}
input,button{font-size:1em;margin:0.5em}</style></head><body>
<h2>CactusOS — przesyłanie plików do terminala</h2>
<form method="POST" action="/upload" enctype="multipart/form-data">
  <input type="file" name="file" required>
  <button type="submit">Wyślij</button>
</form>
<p>Pliki trafiają do katalogu <code>uploads/</code> w workspace.</p>
<p>System może je czytać i generować na ich podstawie.</p>
</body></html>"""
            body = html.encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self._json(404, {"ok": False, "error": "not_found"})

    def do_POST(self) -> None:
        parsed = urlparse(self.path)
        if parsed.path != "/upload":
            self._json(404, {"ok": False, "error": "not_found"})
            return

        length = int(self.headers.get("Content-Length", "0"))
        if length > MAX_BYTES:
            self._json(413, {"ok": False, "error": "file_too_large", "max_mb": 20})
            return

        ctype = self.headers.get("Content-Type", "")
        raw = self.rfile.read(length)

        # Simple multipart or raw base64 JSON
        filename = "upload.bin"
        data = raw

        if "multipart/form-data" in ctype:
            # Minimal multipart parse for single file
            boundary = ctype.split("boundary=")[-1].strip()
            parts = raw.split(("--" + boundary).encode())
            for part in parts:
                if b"filename=" in part:
                    header, _, body = part.partition(b"\r\n\r\n")
                    for line in header.split(b"\r\n"):
                        if b"filename=" in line:
                            fn = line.split(b"filename=")[-1].strip(b'"\r')
                            filename = fn.decode("utf-8", errors="replace") or filename
                    data = body.rstrip(b"\r\n--")
                    break
        elif "application/json" in ctype:
            try:
                payload = json.loads(raw.decode("utf-8"))
                filename = payload.get("filename", filename)
                data = base64.b64decode(payload.get("content_b64", ""))
            except Exception as e:
                self._json(400, {"ok": False, "error": str(e)})
                return

        safe_name = Path(filename).name
        dest = UPLOAD_DIR / safe_name
        dest.write_bytes(data)
        self._json(200, {
            "ok": True,
            "saved": str(dest),
            "size": len(data),
            "message": "Plik zapisany. System może go teraz czytać.",
        })

    def log_message(self, fmt: str, *args: Any) -> None:
        print(f"[file_transfer] {fmt % args}")


def serve(host: str = "127.0.0.1", port: int = 8765) -> None:
    print(f"CactusOS File Transfer: http://{host}:{port}/")
    print(f"Upload dir: {UPLOAD_DIR.resolve()}")
    server = ThreadingHTTPServer((host, port), FileTransferHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("Stopped.")


if __name__ == "__main__":
    serve()
