# Aerial

A single Rust binary for AI agent-to-agent messaging — peer-to-peer, durable,
and resumable, without requiring a broker, database, or cloud account.
Tagline: **"bridge the agentic gap."**

This file exists so a fresh agent picking up this repo has the context that
isn't obvious from the files alone.

## Status

Aerial is implemented as a single `aerial` Rust binary, in active development
toward v0.2. What exists today:

- a local daemon over a Unix-domain socket (AF_UNIX — via the `uds_windows`
  crate on Windows, so it builds and runs on Linux, macOS, and Windows)
- durable per-agent JSONL mailboxes and an append-only transcript
- name-addressed, at-least-once delivery that survives an offline recipient
- wake notifications so an agent can be woken instead of polling
  (`aerial watch`, with an optional `--exec` hook)
- an MCP adapter (`aerial mcp`) over the same daemon protocol
- a CLI covering register / send / read / ack / history

Read `ARCHITECTURE.md` first for the envelope format, addressing, delivery
model, and open questions. `CHANGELOG.md` tracks released behavior. The
`site/index.html` landing page is deliberate retro brand identity — see
"The website is a bit" below before touching it.

## Working as an agent (messaging over Aerial)

If you are an AI agent collaborating through Aerial, treat your mailbox like an
inbox you must keep watching.

**Rule: keep an inbox open — do not go dark between actions.** Hold a shell
open on your own name so you notice messages as they arrive, and acknowledge
what you handle:

```sh
# preferred: get woken on arrival, no polling
aerial watch <your-name>

# or run a command per message (the process reads and acks its own inbox)
aerial watch <your-name> --exec "<command>"

# fallback if you cannot hold a stream open: poll your inbox
aerial read <your-name>
```

Always `aerial ack --agent <your-name> <envelope-id>` once you have handled a
message so it does not resurface. The mailbox is the source of truth; a wake
event is only a notification that mail is waiting, so a missed or duplicated
wake never loses a message.

### Command reference

The daemon must be running (`aerial up`); every client command talks to it over
the socket (default `.aerial/aerial.sock`, override with `--socket`). Short
aliases are for day-to-day agent use; canonical names in parentheses.

| Command | Purpose |
| --- | --- |
| `aerial up` (`serve`) | Run the local daemon. |
| `aerial join <name>` (`register`) | Register an agent name. |
| `aerial send --from <a> --to <b> --body <text>` (`tell`) | Send a message; `--in-reply-to <id>` keeps lineage. |
| `aerial read <name>` (`inbox`) | List an agent's pending (unacked) messages. |
| `aerial ack --agent <name> <id>` (`done`) | Acknowledge a handled message. |
| `aerial log [--limit N] [--json]` (`history`) | Show message history. |
| `aerial watch <name>` | Stream JSONL wake events as mail arrives. |
| `aerial watch <name> --exec <cmd>` | Run a hook per arrival (`AERIAL_AGENT`, `AERIAL_MESSAGE_ID`, `AERIAL_SOCKET` set in its env). |
| `aerial mcp` | Serve the MCP adapter over stdio (tools: register, tell, inbox, done, history). |

### How it works (briefly)

Each agent is addressed by name. A message is an envelope (`tool_use`-shaped:
id, from, to, optional `in_reply_to`, payload) appended to the recipient's
durable JSONL mailbox, so mail to an offline agent waits until it reads and
acks. Delivery is at-least-once, so handlers should be idempotent. `watch`
subscribes to the daemon and is notified when the recipient gets mail — a
convenience over polling, never a replacement for the durable mailbox.

## The core idea

Three explicit design principles carried through from the architecture
discussion, in priority order:

1. **No required infrastructure.** One binary. No Kafka, no Redis, no
   Postgres needed to get two agents talking. (This was a deliberate
   reaction against over-engineering agent orchestration with heavyweight
   message brokers — see "Naming and vibe history" for why that joke keeps
   coming up.)
2. **Durable by default.** Messages to an offline agent wait in a mailbox,
   they don't vanish.
3. **Resumable context.** An agent can go idle and pick a conversation back
   up later with real prior context, via transcript replay — not just a bare
   new message.

Full detail — envelope format, addressing scheme, delivery model, transport,
open questions — is in `ARCHITECTURE.md`. Don't re-derive it; read it.

## Research grounding — confidence levels matter here

The architecture was informed by researching how Claude Code itself does
inter-agent communication (subagents, and the experimental "Agent Teams"
feature). Two confidence tiers came out of that research, and the doc
preserves the distinction on purpose:

- **High confidence / verifiable:** the `tool_use`/`tool_result` message
  envelope shape, parent/lineage tracking on responses, and transcript-replay
  session resumption. These are real and are the basis for Aerial's own
  envelope design.
- **Low confidence / inferred:** exact file layouts, locking mechanisms, and
  internal function names that came back from a research pass. Treat
  anything this specific about Claude Code's *undocumented* internals as
  unverified — it was flagged as inference dressed up as fact, not copied
  into Aerial's design as ground truth.

If you're extending the architecture, keep citing what's actually confirmed
vs. what's a design choice Aerial is making on its own.

## The website is a bit — don't "fix" it

`site/index.html` is **deliberately** styled like a 2010s static
website / an exported PowerPoint deck: skeuomorphic glossy buttons, drop
shadows, a scrolling marquee ticker, an Arial wordmark (yes, literal Arial,
on purpose — it's one letter off from the project name), a fake hit counter,
and copy structured as six literal "slides" (Title, Agenda, Problem,
Solution, Roadmap, Q&A) including a joke "Live Demo — skipped for time"
callback. This is intentional brand identity, not a rough draft of a modern
landing page. If asked to redesign the site, confirm first whether the
intent is "make the real thing" vs. "keep committing to the bit" — don't
default to modernizing it.

The logo is an antenna (a Yagi/TV aerial, matching the name), rendered as
inline SVG in both `site/index.html` and referenced conceptually in
`ARCHITECTURE.md`.

## Naming and vibe history (why "Aerial", why not X)

- Rejected **"Consensus"** — collides with goconsensus.com (an actual AI
  agent product), consensus.app (academic search engine), and a CoinDesk
  crypto conference brand.
- Rejected **"Yapp"** — collides with yapp.us, a funded event-app company
  since 2011.
- Landed on **Aerial** — antenna pun (agents receiving/relaying signals),
  known collisions worth being aware of: github.com/AerialScreensaver/Aerial
  (unrelated, active OSS macOS screensaver) and airia.com (an actual AI
  agent orchestration platform, one letter off). Neither is blocking, both
  are worth knowing about for discoverability (SEO/GitHub search) reasons.
- Running joke throughout early ideation: "Apache Kafka for agent
  orchestration" as the wrong, overbuilt answer — this is *why* design
  principle #1 above ("no required infrastructure") is stated so bluntly.
  It's a direct reaction to that instinct, not an arbitrary constraint.

## Monetization direction (if it comes up)

Open-core: the binary and protocol stay fully open source. The paid layer,
if/when pursued, is a hosted ops/observability layer (a dashboard — since
the binary itself is intentionally headless/UI-less) plus enterprise
features (SSO, RBAC, audit logs) and support contracts. Not urgent, just
recorded so it's not re-litigated from scratch.

## Next steps

- v0.2 wraps up the wake (`watch` / `--exec`), MCP adapter, Windows support,
  and docs/tests work; the remaining step is cutting the v0.2 Homebrew release
  (version bump, `CHANGELOG.md`, and the tap formula under
  `packaging/homebrew/`).
- Open design questions are listed at the bottom of `ARCHITECTURE.md`
  (exactly-once vs. at-least-once delivery, whether the registry persists
  across daemon restarts, how much of the wire format to version from v0).
