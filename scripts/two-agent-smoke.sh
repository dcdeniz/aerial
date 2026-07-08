#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

cargo build --quiet

./target/debug/aerial serve --data-dir "${DATA_DIR}" &
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

./target/debug/aerial register --socket "${SOCKET}" engineer >/dev/null
./target/debug/aerial register --socket "${SOCKET}" researcher >/dev/null

./target/debug/aerial tell \
  --socket "${SOCKET}" \
  --from engineer \
  --to researcher \
  --body "Agent 2 smoke test: please confirm mailbox delivery." >/dev/null

INBOX="$(./target/debug/aerial inbox --socket "${SOCKET}" researcher)"
ENVELOPE_ID="$(printf '%s\n' "${INBOX}" | sed -n 's/.*"id": "\([^"]*\)".*/\1/p' | head -n 1)"

if [[ -z "${ENVELOPE_ID}" ]]; then
  echo "expected one pending envelope, got:" >&2
  printf '%s\n' "${INBOX}" >&2
  exit 1
fi

./target/debug/aerial done --socket "${SOCKET}" --agent researcher "${ENVELOPE_ID}" >/dev/null

PENDING_AFTER_ACK="$(./target/debug/aerial inbox --socket "${SOCKET}" researcher)"
if ! printf '%s\n' "${PENDING_AFTER_ACK}" | grep -q '"envelopes": \[\]'; then
  echo "expected empty mailbox after ack, got:" >&2
  printf '%s\n' "${PENDING_AFTER_ACK}" >&2
  exit 1
fi

./target/debug/aerial history --socket "${SOCKET}" --limit 5
