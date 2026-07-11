#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${AERIAL_BIN:-aerial}"
mkdir -p "${ROOT}/target"
DATA_DIR="$(mktemp -d "${ROOT}/target/aerial-smoke.XXXXXX")"
DAEMON_PID=""

cleanup() {
  if [[ -n "${DAEMON_PID}" ]] && kill -0 "${DAEMON_PID}" 2>/dev/null; then
    kill "${DAEMON_PID}" 2>/dev/null || true
    wait "${DAEMON_PID}" 2>/dev/null || true
  fi
  rm -rf "${DATA_DIR}"
}
trap cleanup EXIT

cd "${ROOT}"

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

"${BIN}" register --socket "${SOCKET}" engineer >/dev/null
"${BIN}" register --socket "${SOCKET}" researcher >/dev/null

"${BIN}" tell \
  --socket "${SOCKET}" \
  --from engineer \
  --to researcher \
  --body "Agent 2 smoke test: please confirm mailbox delivery." >/dev/null

INBOX="$("${BIN}" inbox --socket "${SOCKET}" researcher)"
ENVELOPE_ID="$(printf '%s\n' "${INBOX}" | sed -n 's/.*"id": "\([^"]*\)".*/\1/p' | head -n 1)"

if [[ -z "${ENVELOPE_ID}" ]]; then
  echo "expected one pending envelope, got:" >&2
  printf '%s\n' "${INBOX}" >&2
  exit 1
fi

"${BIN}" done --socket "${SOCKET}" --agent researcher "${ENVELOPE_ID}" >/dev/null

PENDING_AFTER_ACK="$("${BIN}" inbox --socket "${SOCKET}" researcher)"
if ! printf '%s\n' "${PENDING_AFTER_ACK}" | grep -q '"envelopes": \[\]'; then
  echo "expected empty mailbox after ack, got:" >&2
  printf '%s\n' "${PENDING_AFTER_ACK}" >&2
  exit 1
fi

"${BIN}" history --socket "${SOCKET}" --limit 5
