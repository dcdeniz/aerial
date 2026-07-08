# Aerial

A single Rust binary for AI agent-to-agent messaging — peer-to-peer, durable,
and resumable, without requiring a broker, database, or cloud account.
Tagline: **"bridge the agentic gap."**

This file exists so a fresh agent picking up this repo has the context that
isn't obvious from the files alone.

## Status (as of this writing)

- Repo initialized (`git init`), **nothing committed yet**.
- `ARCHITECTURE.md` — the actual design doc. Read this first.
- `site/index.html` — a landing page. See "The website is a bit" below
  before touching it.
- **No Rust code exists yet.** No `Cargo.toml`, no crate structure, nothing
  scaffolded. The architecture doc is a design, not an implementation.
- A local dev server (`python3 -m http.server 8080` from `site/`) may or may
  not still be running in the background from an earlier session — it's not
  part of the project, just a way to preview the page. Don't treat it as
  infrastructure.

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

## Next steps (open as of this writing)

- No Rust code. First real implementation work would be scaffolding the
  crate(s) per `ARCHITECTURE.md`'s core concepts (Agent, Envelope, Mailbox,
  Transcript, Registry).
- Nothing is committed to git yet — `ARCHITECTURE.md`, `site/index.html`,
  and this file are all currently untracked/uncommitted.
- Open design questions are listed at the bottom of `ARCHITECTURE.md`
  (exactly-once vs. at-least-once delivery, whether the registry persists
  across daemon restarts, how much of the wire format to version from v0).
