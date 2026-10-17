#!/usr/bin/env bash
# Acquire and verify the pinned MVP inference runtime and model (MVP-B §6–§10, §77–§80).
#
#   llama.cpp  v0.7.0 = build tag b11669 = commit 620d2225cc436ab0b001e3200e590e3b286ef9ab  (MIT)
#   model      Qwen3-4B-Instruct-2507-Q4_K_M.gguf from unsloth/Qwen3-4B-Instruct-2507-GGUF
#              @ a06e946bb6b655725eafa393f4a9745d460374c9                                     (Apache-2.0)
#
# Every downloaded artifact is checked against a SHA-256 fixed in this file
# before it is used; TLS alone is not trusted. A mismatch moves the file to
# quarantine/ and exits non-zero. Nothing is written inside the repository:
# the cache is $MBONGO_INFER_HOME (default ~/.cache/mbongo/inference).
# Re-running is idempotent: verified artifacts are not downloaded again.
# After this script, inference runs offline.
set -euo pipefail

HOME_DIR="${MBONGO_INFER_HOME:-$HOME/.cache/mbongo/inference}"
TAG="b11669"
RELEASE_BASE="https://github.com/ggml-org/llama.cpp/releases/download/${TAG}"

MODEL_FILE="Qwen3-4B-Instruct-2507-Q4_K_M.gguf"
MODEL_URL="https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/a06e946bb6b655725eafa393f4a9745d460374c9/${MODEL_FILE}"
MODEL_SHA256="3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597"
MODEL_SIZE=2497281120

case "$(uname -s)-$(uname -m)" in
  MINGW*-x86_64|MSYS*-x86_64|CYGWIN*-x86_64)
    ARCHIVE="llama-${TAG}-bin-win-cpu-x64.zip"
    ARCHIVE_SHA256="37b3efcf962847ca869745fd4ccf7f5e7e8dacfeff9fa3523b4ca8cdc66d632b"
    RUNTIME_DIR="${HOME_DIR}/runtime/${TAG}-win-cpu-x64"
    SERVER_REL="llama-server.exe"
    SERVER_SHA256="6a4635aeb35656153d4bfeeddf761d24ed84aada9c9108d580737cbed465141b"
    ;;
  Linux-x86_64)
    ARCHIVE="llama-${TAG}-bin-ubuntu-x64.tar.gz"
    ARCHIVE_SHA256="cf07c1c112d709bd67e522017471de57ed0ec8bea031d732c5422351c07f65ac"
    RUNTIME_DIR="${HOME_DIR}/runtime/${TAG}-ubuntu-x64"
    SERVER_REL="llama-${TAG}/llama-server"
    SERVER_SHA256="3f2b53013c2eb596c8aca87e4740240a41f95bcebf72e61084d6d5ba0db761cc"
    ;;
  *)
    echo "FAIL: no pinned CPU build for $(uname -s)-$(uname -m)" >&2
    exit 2
    ;;
esac

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}' | tr -d '*'
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

quarantine() {
  mkdir -p "${HOME_DIR}/quarantine"
  mv -f "$1" "${HOME_DIR}/quarantine/$(basename "$1").$(date +%s)"
  echo "FAIL: $2 — moved to ${HOME_DIR}/quarantine" >&2
  exit 1
}

mkdir -p "${HOME_DIR}/runtime" "${HOME_DIR}/models" "${HOME_DIR}/logs"

# ── runtime ─────────────────────────────────────────────────────────────
SERVER="${RUNTIME_DIR}/${SERVER_REL}"
if [ -f "$SERVER" ] && [ "$(sha256_of "$SERVER")" = "$SERVER_SHA256" ]; then
  echo "runtime: present and verified (${SERVER_REL})"
else
  ARCHIVE_PATH="${HOME_DIR}/runtime/${ARCHIVE}"
  if [ ! -f "$ARCHIVE_PATH" ] || [ "$(sha256_of "$ARCHIVE_PATH")" != "$ARCHIVE_SHA256" ]; then
    echo "runtime: downloading ${ARCHIVE}"
    curl -L --fail --retry 3 -sS -o "${ARCHIVE_PATH}.part" "${RELEASE_BASE}/${ARCHIVE}"
    got="$(sha256_of "${ARCHIVE_PATH}.part")"
    [ "$got" = "$ARCHIVE_SHA256" ] || quarantine "${ARCHIVE_PATH}.part" "archive SHA-256 ${got} != ${ARCHIVE_SHA256}"
    mv -f "${ARCHIVE_PATH}.part" "$ARCHIVE_PATH"
  fi
  rm -rf "$RUNTIME_DIR"; mkdir -p "$RUNTIME_DIR"
  case "$ARCHIVE" in
    *.zip) unzip -q -o "$ARCHIVE_PATH" -d "$RUNTIME_DIR" ;;
    *.tar.gz) tar -xzf "$ARCHIVE_PATH" -C "$RUNTIME_DIR" ;;
  esac
  got="$(sha256_of "$SERVER")"
  if [ "$got" != "$SERVER_SHA256" ]; then
    echo "FAIL: llama-server SHA-256 ${got} != ${SERVER_SHA256}" >&2
    rm -rf "$RUNTIME_DIR"
    exit 1
  fi
  echo "runtime: extracted and verified"
fi
"$SERVER" --version 2>&1 | grep -q "build 11669, commit 620d2225c" \
  || { echo "FAIL: llama-server does not report build 11669, commit 620d2225c" >&2; exit 1; }

# ── model ───────────────────────────────────────────────────────────────
MODEL="${HOME_DIR}/models/${MODEL_FILE}"
if [ -f "$MODEL" ]; then
  echo "model: present; verifying SHA-256 (about ${MODEL_SIZE} bytes)"
  got="$(sha256_of "$MODEL")"
  [ "$got" = "$MODEL_SHA256" ] || quarantine "$MODEL" "model SHA-256 ${got} != ${MODEL_SHA256}"
else
  echo "model: downloading ${MODEL_FILE} (${MODEL_SIZE} bytes)"
  curl -L --fail --retry 3 -sS -o "${MODEL}.part" "$MODEL_URL"
  size="$(wc -c < "${MODEL}.part" | tr -d ' ')"
  [ "$size" = "$MODEL_SIZE" ] || quarantine "${MODEL}.part" "model size ${size} != ${MODEL_SIZE}"
  got="$(sha256_of "${MODEL}.part")"
  [ "$got" = "$MODEL_SHA256" ] || quarantine "${MODEL}.part" "model SHA-256 ${got} != ${MODEL_SHA256}"
  mv -f "${MODEL}.part" "$MODEL"
fi

echo "MVP_INFERENCE_SETUP=PASS"
echo "runtime=${SERVER}"
echo "model=${MODEL}"
