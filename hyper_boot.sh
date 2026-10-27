#!/usr/bin/env bash
set -euo pipefail

OLLAMA_URL="http://127.0.0.1:11434"
OLLAMA_LOG="${TMPDIR:-/tmp}/hyperos-ollama-serve.log"

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
    printf 'Started Ollama daemon (pid %s); waiting for %s\n' "$OLLAMA_PID" "$OLLAMA_URL"
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

if ! ollama list | grep -q '^llama3.2-vision'; then
    ollama pull llama3.2-vision
fi
ollama run llama3.2-vision "Reply with OK." >/dev/null 2>&1 &
WARMUP_PID=$!
wait "$WARMUP_PID"

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' "error: cargo is not installed" >&2
    exit 1
fi

if [[ -n "${HYPEROS_CARGO_TARGET:-}" ]]; then
    cargo run --release --features baremetal --target "$HYPEROS_CARGO_TARGET" -- "$@"
else
    cargo run --release --features baremetal -- "$@"
fi