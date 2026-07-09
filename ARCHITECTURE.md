# Aerial — Architecture (v0 draft)

Aerial is a single Rust binary that lets AI agents message each other directly:
peer-to-peer, durable, and resumable — without requiring a message broker,
a database, or a cloud account to get started.

This document describes the initial design and the reasoning behind it.

## Design principles

1. **No required infrastructure.** `aerial` should be one binary. Two agents
   should be able to talk to each other within a minute of installing it — no
   Kafka, no Redis, no Postgres to stand up first.
2. **Durable by default.** A message sent to an offline agent is not lost. It
   waits in that agent's mailbox until the agent reconnects.
3. **Resumable context.** An agent that's been idle for an hour can pick a
   conversation back up with its prior context intact — not just receive a
   bare new message with no history.
4. **Process isolation.** Each agent is its own OS process with its own crash
   domain. A bug in one agent cannot corrupt another agent's state.
5. **Boring transport, opinionated protocol.** The delivery guarantees and
   message shape are where Aerial should have a real point of view. The bytes
   on the wire should be the simplest mechanism that works — sockets and files
   before anything fancier.

## Prior art: what Claude Code actually does

Before designing Aerial's protocol, we looked at how Claude Code's own
multi-agent features (subagents, and the experimental "Agent Teams") handle
this, since it's the most direct existence proof available. Confidence
varies by claim:

**High confidence (documented/verifiable):**
- Inter-agent messages ride on the same `tool_use` / `tool_result` block
  format the Claude API uses for all tool calls — a message is, structurally,
  a tool call with an `id`, a `name`, and a JSON `input`/`content` payload.
- Responses carry a parent/lineage reference back to the call that spawned
  them, so a tree of agent calls stays traceable.
- Session/agent context is persisted as an append-only transcript and
  resumed by replaying it — not by snapshotting arbitrary in-memory state.
- Regular subagents are parent→child only: a subagent reports back to
  whoever spawned it, and cannot message a sibling directly.
- "Agent Teams" (experimental) is the one feature that does true peer
  messaging — named agents send each other messages directly via a
  mailbox-style tool, delivered automatically without the recipient polling.

**Low confidence (inferred, not confirmed — treat as inspiration, not spec):**
- Exact on-disk file layout, locking mechanism for task claiming, and the
  wire format of the mailbox itself are not published. Anything this
  specific from secondary research should not be copied as ground truth into
  Aerial's own implementation.

The two takeaways worth keeping: (1) a **tool-call-shaped envelope** with
lineage tracking is a genuinely good, proven message shape, and (2) the
"peer-to-peer" feature that exists is closer to a **durable, name-addressed
mailbox** than to a broker or pub/sub bus. Neither uses anything like Kafka —
so Aerial doesn't need to either.

## Core concepts

- **Agent** — one OS process, registered with the local Aerial daemon under
  a name.
- **Envelope** — the message unit. Modeled loosely on `tool_use`/`tool_result`:
  an id, sender, recipient, optional reply-to, a kind, and a JSON payload.
- **Mailbox** — a durable, per-agent inbox. Messages persist here until
  delivered and acknowledged.
- **Transcript** — an append-only log per agent that makes resumption
  possible: to resume an agent, replay its transcript rather than trying to
  snapshot live state.
- **Registry** — a lightweight name → process mapping, scoped to one local
  "swarm" (a set of agents run together), not a global directory.

## Addressing

Name-based, not PID- or UUID-based — `send("researcher", ...)`, not
`send(pid_4821, ...)`. Names resolve through the local registry the daemon
holds in memory (and checkpoints to disk for restart). If an agent process
dies and a new one claims the same name, the newest registration wins;
senders don't need to know a process restarted underneath a name.

Raw agent IDs (UUIDs) remain available for the rare case where two
processes legitimately want the same human-readable name disambiguated.

## Message envelope

```rust
struct Envelope {
    id: Uuid,
    from: AgentId,
    to: AgentId,
    in_reply_to: Option<Uuid>,
    kind: MessageKind,       // Message, Ack, Resume, TaskClaim
    payload: serde_json::Value,
    sent_at: u64,            // unix millis, set by the daemon, not the sender
}
```

`in_reply_to` gives the same lineage tracking that made `parent_tool_use_id`
useful in tool-call chains — you can always reconstruct a conversation as a
tree, not just a flat log.

## Delivery model

Hybrid: **push when possible, durable mailbox always.**

- If the recipient is connected, the daemon pushes the envelope immediately
  over its socket connection.
- If not, the envelope is appended to the recipient's on-disk mailbox
  (JSONL) and delivered the moment that agent reconnects.
- Every delivered envelope is acknowledged; unacknowledged envelopes are
  redelivered on reconnect. At-least-once, not exactly-once, at v0.

This avoids the two failure modes of picking one model outright: pure
push-only (message lost if the recipient isn't listening) and pure
poll-based (recipient has to ask, adds latency, easy to get wrong).

## Resumability

An agent's mailbox and transcript are the same append-only JSONL file on
disk. Resuming an agent means:

1. Start (or reattach to) the agent process.
2. Replay its transcript to reconstruct context.
3. Register it under its name with the daemon.
4. Deliver anything that queued in its mailbox while it was gone.

No separate snapshot/checkpoint format at v0 — replay-from-log is simpler
and is the same strategy Claude Code's own session resumption uses.

## Concurrency & isolation

Each agent is a separate OS process. The Aerial daemon is the only shared
component, and it only ever touches the registry and mailboxes — never an
agent's internal state. A crash in one agent process cannot corrupt
another's mailbox or transcript, since each is its own file with its own
process as sole writer.

## Transport

- **v0**: Unix domain socket (named pipe on Windows) between each agent
  process and the local Aerial daemon, one daemon per machine. Mailboxes and
  transcripts are plain JSONL files on disk.
- **aerial-local**: the local development product. This is the v0 shape:
  local daemon, local mailbox files, local transcript history, and a CLI/MCP
  surface agents can use from the same machine.
- **aerial-server**: the cross-computer product. This is the later networked
  shape: agents on different machines can exchange the same envelope-shaped
  messages through a server/daemon mode. It should preserve the local design's
  durability and resumability guarantees rather than becoming generic pub/sub.

## Non-goals for v0

- Distributed consensus or multi-node clustering.
- Auth / multi-tenant security model.
- Language bindings beyond Rust. A CLI plus a documented local socket
  protocol is enough for other languages to speak to it if needed later.

## Open questions

- Exactly-once delivery: worth the complexity later, or does at-least-once
  plus idempotent handlers cover real use cases?
- Should the registry ever persist across daemon restarts, or is a swarm
  inherently ephemeral (dies with the daemon that started it)?
- How much of the mailbox/transcript format should be a documented,
  versioned wire protocol from day one, versus an internal detail we're
  free to change before v1?
