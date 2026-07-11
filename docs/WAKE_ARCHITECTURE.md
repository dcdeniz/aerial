# Wake Architecture

Aerial should support agents that wake when messages arrive without keeping an
LLM session active.

## Principle

The mailbox stays the source of truth. Wake-up is only a notification that a
pending envelope exists.

```text
sender -> daemon -> durable mailbox -> wake notification -> agent wrapper
```

If notification delivery fails, the message remains pending until the agent
reads and acknowledges it.

## Socket Watch

An agent runtime, wrapper, or supervisor may keep a cheap socket subscription
open with the daemon:

```sh
aerial watch agent2
```

The daemon emits small JSONL events when new envelopes arrive:

```json
{"event":"message","agent":"agent2","id":"..."}
```

Keeping this socket open must not imply keeping an LLM context active.

## Exec Hook

Aerial may also provide a process wake hook:

```sh
aerial watch agent2 --exec "codex ..."
```

The hook runs after a new pending message is appended. The started process is
responsible for reading its inbox and acknowledging handled envelopes.

## Agent Supervisor

The launch-facing runtime is the supervisor:

```sh
aerial agent exec agent2 -- ./handle-message.sh
aerial agent codex agent2 --cd .
```

The supervisor uses the same watch path, but owns the full handler lifecycle:

```text
daemon -> durable mailbox -> watch event -> supervisor -> worker command -> ack on success
```

This is intentionally different from driving an arbitrary already-open
terminal. Aerial can reliably wake and control processes it owns. For v0.3,
that means running a fresh worker command per message. A later PTY/tmux runtime
can make the worker visible in a persistent terminal while preserving the same
mailbox and ack semantics.

Worker commands receive:

- `AERIAL_AGENT`
- `AERIAL_MESSAGE_ID`
- `AERIAL_MESSAGE_BODY`
- `AERIAL_SOCKET`
- `AERIAL_ENVELOPE_JSON`

If the worker exits successfully, the supervisor acknowledges the envelope. If
the worker fails, the envelope remains pending and will be replayed to the next
watcher.

## MCP

MCP should adapt the daemon protocol. It should not create separate mailbox
state. If MCP supports subscription-style behavior, it should map to the same
watch notification path.
