#!/usr/bin/env bash
# Start local file transfer server for CactusOS terminal
# Files uploaded go to ./uploads/ and can be used by the system
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
mkdir -p uploads

echo "=== CactusOS File Transfer ==="
echo "Upload dir: $ROOT/uploads"
echo "Open in browser: http://127.0.0.1:8765/"
echo "Or curl:"
echo "  curl -F 'file=@twoj_plik.txt' http://127.0.0.1:8765/upload"
echo ""
python3 -m si_1_core.file_transfer
