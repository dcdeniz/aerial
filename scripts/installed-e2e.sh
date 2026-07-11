#!/usr/bin/env bash
set -euo pipefail

BIN="${AERIAL_BIN:-aerial}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "${ROOT}/target"
DATA_DIR="$(mktemp -d "${ROOT}/target/aerial-installed-e2e.XXXXXX")"
DAEMON_PID=""

cleanup() {
  if [[ -n "${DAEMON_PID}" ]] && kill -0 "${DAEMON_PID}" 2>/dev/null; then
    kill "${DAEMON_PID}" 2>/dev/null || true
    wait "${DAEMON_PID}" 2>/dev/null || true
  fi
  rm -rf "${DATA_DIR}"
}
trap cleanup EXIT

if ! command -v "${BIN}" >/dev/null 2>&1 && [[ ! -x "${BIN}" ]]; then
  echo "aerial binary not found. Install aerial-local or set AERIAL_BIN=/path/to/aerial." >&2
  exit 1
fi

"${BIN}" serve --data-dir "${DATA_DIR}" &
DAEMON_PID="$!"

for _ in {1..50}; do
  if [[ -S "${DATA_DIR}/aerial.sock" ]]; then
    break
  fi
  sleep 0.1
done

if [[ ! -S "${DATA_DIR}/aerial.sock" ]]; then
  echo "daemon socket did not appear at ${DATA_DIR}/aerial.sock" >&2
  exit 1
fi

SOCKET="${DATA_DIR}/aerial.sock"

EXCHANGE="$("${BIN}" exchange \
  --socket "${SOCKET}" \
  --from engineer \
  --to agent2 \
  --body "Installed binary e2e: confirm exchange macro." \
  --json)"

if ! printf '%s\n' "${EXCHANGE}" | grep -q '"from": "engineer"'; then
  echo "exchange macro did not return the sender:" >&2
  printf '%s\n' "${EXCHANGE}" >&2
  exit 1
fi

STATUS="$("${BIN}" status --socket "${SOCKET}" agent2 --json)"
if ! printf '%s\n' "${STATUS}" | grep -q '"body": "Installed binary e2e: confirm exchange macro."'; then
  echo "status macro did not show the pending message:" >&2
  printf '%s\n' "${STATUS}" >&2
  exit 1
fi

DRAIN="$("${BIN}" drain --socket "${SOCKET}" agent2 --json)"
if ! printf '%s\n' "${DRAIN}" | grep -q '"acked"'; then
  echo "drain macro did not ack pending mail:" >&2
  printf '%s\n' "${DRAIN}" >&2
  exit 1
fi

AFTER="$("${BIN}" status --socket "${SOCKET}" agent2 --json)"
if ! printf '%s\n' "${AFTER}" | grep -q '"pending": \[\]'; then
  echo "expected empty mailbox after drain:" >&2
  printf '%s\n' "${AFTER}" >&2
  exit 1
fi

"${BIN}" history --socket "${SOCKET}" --limit 5
