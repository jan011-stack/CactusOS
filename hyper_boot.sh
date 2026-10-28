#!/usr/bin/env bash
# HyperOS boot — Ollama ECO by default
set -euo pipefail

OLLAMA_URL="${OLLAMA_URL:-http://127.0.0.1:11434}"
OLLAMA_LOG="${TMPDIR:-/tmp}/hyperos-ollama-serve.log"
export OLLAMA_NUM_PARALLEL="${OLLAMA_NUM_PARALLEL:-1}"
export OLLAMA_MAX_LOADED_MODELS="${OLLAMA_MAX_LOADED_MODELS:-1}"
export OLLAMA_KEEP_ALIVE="${OLLAMA_KEEP_ALIVE:-0}"
export OLLAMA_FLASH_ATTENTION="${OLLAMA_FLASH_ATTENTION:-1}"
export CACTUS_OLLAMA_PROFILE="${CACTUS_OLLAMA_PROFILE:-eco}"
export CACTUS_OLLAMA_MODEL="${CACTUS_OLLAMA_MODEL:-llama3.1:8b}"
SKIP_WARMUP="${HYPEROS_SKIP_WARMUP:-0}"
USE_LITE="${HYPEROS_OLLAMA_LITE:-0}"

if ! command -v ollama >/dev/null 2>&1; then
    printf '%s\n' "error: ollama is not installed" >&2
    exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
    printf '%s\n' "error: curl is required to check the local Ollama service" >&2
    exit 1
fi

if ! curl --silent --fail --max-time 1 "$OLLAMA_URL/api/tags" >/dev/null; then
    ollama serve >"$OLLAMA_LOG" 2>&1 &
    OLLAMA_PID=$!
    printf 'Started Ollama daemon (pid %s, eco limits); waiting for %s\n' "$OLLAMA_PID" "$OLLAMA_URL"
    ready=0
    for _ in {1..30}; do
        if curl --silent --fail --max-time 1 "$OLLAMA_URL/api/tags" >/dev/null; then
            ready=1
            break
        fi
        sleep 1
    done
    if [[ "$ready" != 1 ]]; then
        printf 'error: Ollama did not become ready; see %s\n' "$OLLAMA_LOG" >&2
        exit 1
    fi
fi

if [[ "$USE_LITE" == "1" ]]; then
    if ! ollama list 2>/dev/null | grep -q '^cactus-ai-lite'; then
        ollama pull llama3.2:1b || true
        ollama create cactus-ai-lite -f Modelfile.lite || true
    fi
    MODEL_NAME="cactus-ai-lite"
    export CACTUS_OLLAMA_MODEL="$MODEL_NAME"
else
    if ! ollama list 2>/dev/null | awk 'NR > 1 {print $1}' | grep -Fxq "$CACTUS_OLLAMA_MODEL"; then
        ollama pull "$CACTUS_OLLAMA_MODEL"
    fi
    MODEL_NAME="$CACTUS_OLLAMA_MODEL"
fi

if [[ "$SKIP_WARMUP" != "1" ]]; then
    # Short warmup; keep_alive 0 so weights unload after
    ollama run "$MODEL_NAME" "OK" >/dev/null 2>&1 || true
fi

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' "error: cargo is not installed" >&2
    exit 1
fi

if [[ -n "${HYPEROS_CARGO_TARGET:-}" ]]; then
    cargo run --release --features baremetal --target "$HYPEROS_CARGO_TARGET" -- "$@"
else
    cargo run --release --features baremetal -- "$@"
fi
