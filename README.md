# Aerial

[![CI](https://github.com/dcdeniz/aerial/actions/workflows/ci.yml/badge.svg)](https://github.com/dcdeniz/aerial/actions/workflows/ci.yml)
[![Homebrew](https://github.com/dcdeniz/aerial/actions/workflows/homebrew.yml/badge.svg)](https://github.com/dcdeniz/aerial/actions/workflows/homebrew.yml)
[![npm](https://github.com/dcdeniz/aerial/actions/workflows/npm.yml/badge.svg)](https://github.com/dcdeniz/aerial/actions/workflows/npm.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A single Rust binary for AI agent-to-agent messaging: peer-to-peer, durable,
and resumable without requiring Kafka, Redis, Postgres, or a cloud account.

## Current Status

Aerial currently ships as one Rust binary with:

- a local daemon over a Unix domain socket
- native Windows support through AF_UNIX, distributed for 64-bit Windows as
  the `aerial-local` npm package
- durable per-agent JSONL mailboxes
- an append-only message history transcript
- CLI commands agents can use to register, send, read, ack, and inspect history
- wake notifications so an agent can `watch` its mailbox and be woken when mail
  arrives — optionally running an `--exec` hook — instead of polling
- an agent supervisor so a mailbox message can launch a real worker command,
  including a Codex wrapper

Homebrew packaging is available as `aerial-local`. An MCP adapter over the
daemon protocol is available through the hidden `aerial mcp` stdio subcommand —
see [MCP](#mcp).

Windows is a tested first-class target. The npm release runs the same Rust
binary and validates daemon messaging, MCP stdio, wake notifications, and
supervisor execution on `windows-latest` before publication.

## Roadmap

- **aerial-local**: local agentic development. One machine, one local daemon,
  durable mailboxes, transcript history, and CLI/MCP adapters for agents
  working in the same development environment.
- **aerial-server**: cross-computer agent communication. A server/daemon mode
  for agents on different machines to exchange the same envelope-shaped
  messages without giving up durable mailboxes or resumable history.

## Quickstart

Install the CLI:

```sh
# Windows
npm install --global aerial-local

# macOS
brew tap dcdeniz/aerial
brew trust dcdeniz/aerial
brew install dcdeniz/aerial/aerial-local
```

Run the daemon:

```sh
aerial up
```

In another shell, register two agents:

```sh
aerial join engineer
aerial join researcher
```

Send a message:

```sh
aerial send --from engineer --to researcher --body "Please inspect the architecture."
```

Read the recipient mailbox:

```sh
aerial read researcher
```

Ack a delivered envelope:

```sh
aerial ack --agent researcher <envelope-id>
```

View prompt/message history:

```sh
aerial log --limit 20
```

Or use the bundled flow macros:

```sh
aerial exchange --from engineer --to researcher --body "Please inspect the architecture."
aerial status researcher
aerial drain researcher
```

The default history view is intentionally compact:

```text
Agent 28y49uhrfquf -> Agent 14u1rj13ru1 "Message First 50 Characters ...."
```

Use `--json` on `history` when an agent or tool needs structured output.

The canonical command names still exist as `serve`, `register`, `tell`,
`inbox`, `done`, and `history`; the shorter aliases are meant for day-to-day
agent use.

## MCP

Agents that speak [MCP](https://modelcontextprotocol.io) can drive the daemon
through a hidden stdio adapter:

```sh
aerial mcp --socket .aerial/aerial.sock
```

It exposes primitive tools — `register`, `tell`, `inbox`, `done`, and
`history` — plus flow macros — `status`, `drain`, and `exchange`. Every call is
dispatched to the running daemon, so the adapter keeps no separate mailbox
state; the daemon stays the single source of truth. See [`docs/MCP.md`](docs/MCP.md)
for the tool reference, client configuration, and an example session.

## Waking agents

Delivery is durable and pull-based: a message sent to an agent waits in that
agent's mailbox until the agent reads and acks it. To avoid polling, an agent
(or its supervisor) can keep a cheap connection open and be *woken* when mail
arrives.

Stream arrival events as JSONL:

```sh
aerial watch researcher
```

Each new envelope emits one line:

```json
{"event":"message","agent":"researcher","id":"..."}
```

The mailbox stays the source of truth — an event is only a notification that a
pending envelope exists, so a dropped or duplicated wake never loses a message.
A watcher that attaches while mail is already pending is replayed one event per
waiting envelope, so late subscribers miss nothing.

Run a command on each arrival instead of printing events:

```sh
aerial watch researcher --exec "codex ..."
```

The hook runs through the shell on every new message, with `AERIAL_AGENT`,
`AERIAL_MESSAGE_ID`, and `AERIAL_SOCKET` set in its environment. The spawned
process is responsible for reading its inbox and acking what it handles — the
wake is only the trigger.

## Agent supervisor

For autonomous local work, use the higher-level supervisor. It watches an
agent's mailbox, runs a worker for each pending message, and acknowledges the
message only when the worker exits successfully:

```sh
aerial agent exec researcher -- ./handle-message.sh
```

For Codex, Aerial builds a prompt from the envelope, recent history, and
workspace path:

```sh
aerial agent codex researcher --cd .
```

The supervisor sets `AERIAL_AGENT`, `AERIAL_MESSAGE_ID`,
`AERIAL_MESSAGE_BODY`, `AERIAL_SOCKET`, and `AERIAL_ENVELOPE_JSON` for worker
commands. Use `--once` for deterministic smoke tests or demos:

```sh
aerial agent exec researcher --once -- ./handle-message.sh
```

## Smoke Tests

Run the package-style e2e against `aerial` on PATH:

```sh
scripts/installed-e2e.sh
```

Run the lower-level two-agent exchange against `aerial` on PATH:

```sh
scripts/two-agent-smoke.sh
```

The script starts a temporary daemon, registers `engineer` and `researcher`,
sends one message, acks it, prints compact history, and removes the temporary
data directory.

## Install

On 64-bit Windows with npm:

```powershell
npm install --global aerial-local
aerial --version
```

With Homebrew:

```sh
brew tap dcdeniz/aerial
brew trust dcdeniz/aerial
brew install dcdeniz/aerial/aerial-local
```

See [`docs/INSTALL.md`](docs/INSTALL.md) for packaging details.
