"""Local file transfer + image generation for CactusOS terminal.

- Upload files to uploads/
- Download / list attachments
- Generate images (SVG fallback or external API if configured)
Loopback-only.
"""

from __future__ import annotations

import base64
import json
import os
import urllib.request
from datetime import datetime
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Dict
from urllib.parse import parse_qs, urlparse

UPLOAD_DIR = Path(os.environ.get("CACTUS_UPLOAD_DIR", "./uploads"))
UPLOAD_DIR.mkdir(parents=True, exist_ok=True)
GENERATED_DIR = UPLOAD_DIR / "generated"
GENERATED_DIR.mkdir(parents=True, exist_ok=True)
MAX_BYTES = 20 * 1024 * 1024


def _generate_svg(prompt: str) -> bytes:
    """Simple SVG placeholder when no external image API is set."""
    safe = prompt[:120].replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">
  <rect width="512" height="512" fill="#0d1117"/>
  <text x="256" y="240" text-anchor="middle" fill="#3fb950" font-family="sans-serif" font-size="18">CactusOS</text>
  <text x="256" y="270" text-anchor="middle" fill="#8b949e" font-family="sans-serif" font-size="14">Generated image</text>
  <text x="256" y="300" text-anchor="middle" fill="#c9d1d9" font-family="sans-serif" font-size="12">{safe}</text>
</svg>'''
    return svg.encode("utf-8")


def _try_external_image(prompt: str) -> bytes | None:
    """Optional: call external image API if CACTUS_IMAGE_API_URL is set."""
    url = os.environ.get("CACTUS_IMAGE_API_URL", "").strip()
    key = os.environ.get("CACTUS_IMAGE_API_KEY", "").strip()
    if not url:
        return None
    try:
        body = json.dumps({"prompt": prompt}).encode()
        headers = {"Content-Type": "application/json"}
        if key:
            headers["Authorization"] = f"Bearer {key}"
        req = urllib.request.Request(url, data=body, headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = resp.read()
        return data
    except Exception:
        return None


class FileTransferHandler(BaseHTTPRequestHandler):
    def _json(self, code: int, data: Dict[str, Any]) -> None:
        body = json.dumps(data, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _file_response(self, path: Path, content_type: str = "application/octet-stream") -> None:
        if not path.is_file():
            self._json(404, {"ok": False, "error": "not_found"})
            return
        data = path.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Content-Disposition", f'attachment; filename="{path.name}"')
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        qs = parse_qs(parsed.query)

        if parsed.path == "/files":
            files = []
            for p in sorted(UPLOAD_DIR.rglob("*")):
                if p.is_file():
                    files.append({"name": str(p.relative_to(UPLOAD_DIR)), "size": p.stat().st_size})
            self._json(200, {"ok": True, "files": files, "dir": str(UPLOAD_DIR)})
            return

        if parsed.path.startswith("/download/"):
            rel = parsed.path[len("/download/"):]
            target = (UPLOAD_DIR / rel).resolve()
            if not str(target).startswith(str(UPLOAD_DIR.resolve())):
                self._json(403, {"ok": False, "error": "forbidden"})
                return
            ctype = "image/svg+xml" if target.suffix == ".svg" else "application/octet-stream"
            self._file_response(target, ctype)
            return

        if parsed.path == "/":
            html = """<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>CactusOS Terminal Attachments</title>
<style>body{font-family:monospace;background:#111;color:#eee;padding:2em}
input,button,textarea{font-size:1em;margin:0.4em}</style></head><body>
<h2>CactusOS — załączniki i generowanie obrazów</h2>

<h3>Wyślij plik</h3>
<form method="POST" action="/upload" enctype="multipart/form-data">
  <input type="file" name="file" required>
  <button type="submit">Wyślij</button>
</form>

<h3>Wygeneruj obraz</h3>
<form method="POST" action="/generate-image">
  <textarea name="prompt" rows="3" cols="50" placeholder="Opis obrazu..." required></textarea><br>
  <button type="submit">Generuj</button>
</form>

<p>Pliki: <a href="/files">/files</a></p>
<p>Pobieranie: /download/nazwa_pliku</p>
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
        length = int(self.headers.get("Content-Length", "0"))
        if length > MAX_BYTES:
            self._json(413, {"ok": False, "error": "too_large"})
            return
        raw = self.rfile.read(length)
        ctype = self.headers.get("Content-Type", "")

        if parsed.path == "/upload":
            filename = "upload.bin"
            data = raw
            if "multipart/form-data" in ctype:
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
            safe = Path(filename).name
            dest = UPLOAD_DIR / safe
            dest.write_bytes(data)
            self._json(200, {"ok": True, "saved": str(dest), "size": len(data),
                             "download": f"/download/{safe}"})
            return

        if parsed.path == "/generate-image":
            prompt = ""
            if "application/json" in ctype:
                try:
                    payload = json.loads(raw.decode("utf-8"))
                    prompt = payload.get("prompt", "")
                except Exception as e:
                    self._json(400, {"ok": False, "error": str(e)})
                    return
            else:
                # form-urlencoded or multipart
                try:
                    qs = parse_qs(raw.decode("utf-8"))
                    prompt = qs.get("prompt", [""])[0]
                except Exception:
                    prompt = raw.decode("utf-8", errors="replace")[:500]

            if not prompt.strip():
                self._json(400, {"ok": False, "error": "missing_prompt"})
                return

            img = _try_external_image(prompt)
            ext = ".bin"
            if img is None:
                img = _generate_svg(prompt)
                ext = ".svg"

            ts = datetime.now().strftime("%Y%m%d_%H%M%S")
            name = f"img_{ts}{ext}"
            dest = GENERATED_DIR / name
            dest.write_bytes(img)
            self._json(200, {
                "ok": True,
                "saved": str(dest),
                "download": f"/download/generated/{name}",
                "source": "external" if ext != ".svg" else "svg_fallback",
                "prompt": prompt[:200],
            })
            return

        self._json(404, {"ok": False, "error": "not_found"})

    def log_message(self, fmt: str, *args: Any) -> None:
        print(f"[file_transfer] {fmt % args}")


def serve(host: str = "127.0.0.1", port: int = 8765) -> None:
    print(f"CactusOS Attachments + Image Gen: http://{host}:{port}/")
    print(f"Upload dir: {UPLOAD_DIR.resolve()}")
    print(f"Generated:  {GENERATED_DIR.resolve()}")
    server = ThreadingHTTPServer((host, port), FileTransferHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("Stopped.")


if __name__ == "__main__":
    serve()
