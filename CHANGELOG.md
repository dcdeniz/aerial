# Changelog

All notable changes to Aerial are documented here. This project adheres to
[Semantic Versioning](https://semver.org).

## [0.2.0] - 2026-07-11

Second development release: agents can be *woken* instead of polling, an MCP
adapter exposes the daemon to MCP clients, and the daemon builds and runs on
Windows.

### Added
- **Wake notifications** (#8, #11): the daemon pushes a wake event to any
  connected watcher when a pending envelope arrives, and `aerial watch <agent>`
  streams these as JSONL `{"event":"message","agent":...,"id":...}`. The mailbox
  stays the source of truth, so a dropped or duplicated wake never loses a
  message; a watcher that attaches while mail is already pending is replayed one
  event per waiting envelope.
- **Watch exec hook** (#9): `aerial watch <agent> --exec <cmd>` runs a command
  on each arrival, with `AERIAL_AGENT`, `AERIAL_MESSAGE_ID`, and `AERIAL_SOCKET`
  in its environment; the spawned process reads and acks its own inbox.
- **MCP adapter** (#10): `aerial mcp` speaks MCP over stdio and maps the tools
  `register`, `tell`, `inbox`, `done`, and `history` 1:1 onto daemon requests,
  with no separate mailbox state.
- **Windows support** (#17): the daemon transport uses AF_UNIX sockets via the
  `uds_windows` crate on Windows; a CI matrix builds and tests on Linux, macOS,
  and Windows.
- Documentation and focused tests for the wake, exec, and MCP paths (#12).

## [0.1.0]

Initial release: a single Rust binary with a local daemon over a Unix-domain
socket, durable per-agent JSONL mailboxes, an append-only transcript, and CLI
commands to register, send, read, ack, and inspect history. Homebrew packaging
for macOS.
