# Aerial

A single Rust binary for AI agent-to-agent messaging: peer-to-peer, durable,
and resumable without requiring Kafka, Redis, Postgres, or a cloud account.

Tagline: **bridge the agentic gap.**

## Current Status

Aerial currently ships as one Rust binary with:

- a local daemon over a Unix domain socket
- durable per-agent JSONL mailboxes
- an append-only message history transcript
- CLI commands agents can use to register, send, read, ack, and inspect history

MCP and release packaging are planned, but not implemented yet.

## Quickstart

Run the daemon:

```sh
cargo run -- serve --data-dir .aerial
```

In another shell, register two agents:

```sh
cargo run -- register engineer
cargo run -- register researcher
```

Send a message:

```sh
cargo run -- tell --from engineer --to researcher --body "Please inspect the architecture."
```

Read the recipient mailbox:

```sh
cargo run -- inbox researcher
```

Ack a delivered envelope:

```sh
cargo run -- done --agent researcher <envelope-id>
```

View prompt/message history:

```sh
cargo run -- history --limit 20
```

The default history view is intentionally compact:

```text
Agent 28y49uhrfquf -> Agent 14u1rj13ru1 "Message First 50 Characters ...."
```

Use `--json` on `history` when an agent or tool needs structured output.

## Development Smoke Test

Run a local two-agent exchange end to end:

```sh
scripts/two-agent-smoke.sh
```

The script starts a temporary daemon, registers `engineer` and `researcher`,
sends one message, acks it, prints compact history, and removes the temporary
data directory.

## Install

For local development:

```sh
cargo install --path .
```

Homebrew is the first planned binary/package install path for macOS. See
[`docs/INSTALL.md`](docs/INSTALL.md) for the current packaging plan and the
draft formula template.
