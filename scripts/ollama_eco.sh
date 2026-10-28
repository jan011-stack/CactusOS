#!/usr/bin/env bash
# Start / tune Ollama for low RAM & CPU
set -euo pipefail

export OLLAMA_HOST="${OLLAMA_HOST:-127.0.0.1:11434}"
export OLLAMA_NUM_PARALLEL="${OLLAMA_NUM_PARALLEL:-1}"
export OLLAMA_MAX_LOADED_MODELS="${OLLAMA_MAX_LOADED_MODELS:-1}"
export OLLAMA_KEEP_ALIVE="${OLLAMA_KEEP_ALIVE:-0}"
export OLLAMA_FLASH_ATTENTION="${OLLAMA_FLASH_ATTENTION:-1}"
export CACTUS_OLLAMA_PROFILE="${CACTUS_OLLAMA_PROFILE:-eco}"
export CACTUS_OLLAMA_MODEL="${CACTUS_OLLAMA_MODEL:-llama3.1:8b}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v ollama >/dev/null 2>&1; then
  echo "error: ollama not installed" >&2
  exit 1
fi

if ! curl -sf --max-time 1 "http://${OLLAMA_HOST}/api/tags" >/dev/null; then
  echo "Starting Ollama (eco env)..."
  ollama serve >/tmp/cactus-ollama-eco.log 2>&1 &
  for _ in $(seq 1 30); do
    curl -sf --max-time 1 "http://${OLLAMA_HOST}/api/tags" >/dev/null && break
    sleep 1
  done
fi

MODE="${1:-default}"
case "$MODE" in
  lite)
    ollama create cactus-ai-lite -f Modelfile.lite
    export CACTUS_OLLAMA_MODEL=cactus-ai-lite
    echo "Model: cactus-ai-lite (text-only, lowest RAM)"
    ;;
  balanced)
    ollama create cactus-ai -f Modelfile.balanced
    export CACTUS_OLLAMA_PROFILE=balanced
    export CACTUS_OLLAMA_MODEL=cactus-ai
    echo "Model: cactus-ai balanced"
    ;;
  *)
    ollama pull "$CACTUS_OLLAMA_MODEL"
    echo "Model: $CACTUS_OLLAMA_MODEL eco (default)"
    ;;
esac

echo "Profile=$CACTUS_OLLAMA_PROFILE  PARALLEL=$OLLAMA_NUM_PARALLEL  MAX_LOADED=$OLLAMA_MAX_LOADED_MODELS  KEEP_ALIVE=$OLLAMA_KEEP_ALIVE"
ollama list || true
