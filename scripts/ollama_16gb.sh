#!/usr/bin/env bash
# Best Ollama setup for 16GB RAM (GitHub Codespaces / local)
# Model: llama3.2:3b — good quality, ~2GB RAM, fast enough on CPU
set -euo pipefail

export OLLAMA_HOST="${OLLAMA_HOST:-127.0.0.1:11434}"
export OLLAMA_NUM_PARALLEL=1
export OLLAMA_MAX_LOADED_MODELS=1
export OLLAMA_KEEP_ALIVE="5m"
export OLLAMA_FLASH_ATTENTION=1
export CACTUS_OLLAMA_PROFILE=balanced
export CACTUS_OLLAMA_MODEL=llama3.2:3b

echo "=== CactusOS Ollama 16GB setup ==="
echo "Host: $OLLAMA_HOST"
echo "Model: $CACTUS_OLLAMA_MODEL"

if ! command -v ollama >/dev/null 2>&1; then
  echo "Installing Ollama..."
  curl -fsSL https://ollama.com/install.sh | sh
fi

# Start serve if not running
if ! curl -sf --max-time 2 "http://${OLLAMA_HOST}/api/tags" >/dev/null 2>&1; then
  echo "Starting ollama serve..."
  nohup ollama serve > /tmp/ollama-cactus.log 2>&1 &
  for i in $(seq 1 40); do
    if curl -sf --max-time 1 "http://${OLLAMA_HOST}/api/tags" >/dev/null 2>&1; then
      echo "Ollama ready."
      break
    fi
    echo -n "."
    sleep 1
  done
  echo
fi

echo "Pulling recommended model (llama3.2:3b)..."
ollama pull llama3.2:3b

echo "Creating cactus-ai from Modelfile..."
ollama create cactus-ai -f Modelfile || true

echo "=== Status ==="
ollama list
echo "Profile: balanced | MAX_LOADED=1 | KEEP_ALIVE=5m | FLASH_ATTENTION=1"
echo "Done. Set CACTUS_OLLAMA_MODEL=llama3.2:3b or cactus-ai"
