# Aerial — Agent-Ergonomics Testing Notes

_Hands-on evaluation of `aerial-local` driven the way an autonomous agent would use it,
with recommended changes to make agent-to-agent use smoother._

- **Version tested:** `aerial 0.4.2` (`aerial.exe`)
- **Install:** `npm install --global aerial-local` — succeeded, binary on PATH.
- **Platform:** Windows 11 (npm build). The Windows/npm path now works end to end. ✅

---

## 1. What was tested (all passing)

| Step | Command | Result |
|---|---|---|
| Start daemon | `aerial serve` (`up`) | Daemon starts, creates `.aerial/` (socket + mailboxes) |
| Register | `aerial register engineer` / `register researcher` | `{ "status": "registered", "id": "<uuid>" }` |
| Send | `aerial send --from engineer --to researcher --body "…"` | Envelope returned with `id`, `from`, `to`, `payload`, `sent_at` |
| Inbox | `aerial inbox researcher` | Pending envelope listed (JSON) |
| Status | `aerial status researcher` | `1 pending message(s)` + one-line summary |
| History | `aerial history --limit 5` | Recent messages listed |
| Ack | `aerial done --agent researcher <id>` | `{ "status": "acked" }`; inbox then empty |
| Threaded reply | `aerial send … --in-reply-to <id>` | `in_reply_to` preserved on the envelope |
| Hooks | `watch --exec`, `agent exec`, `agent codex` | Present; wake-driven, auto-ack on success |

**Conclusion:** the core durable-mailbox protocol is solid and cross-platform. The friction
below is all _ergonomics_ — nothing is broken, but several things make it harder than it
needs to be for an agent (or a human supervising one) to use confidently.

---

## 2. Recommended changes (ranked)

### P0 — Names instead of UUIDs in output
You **send** by human name (`--from engineer --to researcher`), but every envelope,
`history`, and `status` line comes back keyed by opaque UUIDs:

```
Agent 02692fff8c45 -> Agent 32c27a61a1f9 "Please inspect the v2 architecture."
```

An agent then has to build and maintain its own UUID↔name map just to interpret replies.

**Change:** include `from_name` / `to_name` on the envelope JSON, and render names (not
truncated UUIDs) in `history`, `status`, and `inbox`. Keep the UUID as a stable id field.

### P0 — Don't silently invent unknown recipients
`aerial send --from engineer --to ghost --body "hi"` — where `ghost` was never registered —
**succeeded** and minted a fresh UUID. A single typo therefore routes messages into a mailbox
no one reads, with no error.

**Change:** default to erroring when the recipient name isn't registered; add an opt-in
`--create` (or `--strict=false`) for the auto-register behaviour.

### P1 — Discovery: list who's in the mesh
There is no `agents` / `who` command. An agent can't enumerate registered peers or their
pending counts, so it can't discover collaborators or detect a dead peer.

**Change:** add `aerial agents` (alias `who`) → names, ids, pending counts, last-seen.

### P1 — Decouple the daemon from the working directory
The socket defaults to `.aerial/aerial.sock` **relative to cwd**, and `serve` keys off
`--data-dir`. Two agents launched from different directories won't find each other unless
every single command threads `--socket`. That is easy to get wrong.

**Change:** honour an `AERIAL_SOCKET` env var and/or default to a home-dir socket
(`~/.aerial/aerial.sock`) so any agent connects with zero flags regardless of cwd.

### P1 — Uniform `--json` and a documented schema
`inbox` returns JSON by default; `status` / `history` are human text unless `--json` is
passed. Mixed output shapes make parsing brittle.

**Change:** support `--json` on every command with a single documented envelope schema;
consider `--json` (machine) vs default (human) consistently across the CLI.

### P2 — Auto-start / crisp "no daemon" error
If `serve` isn't running, commands fail. Make the failure state the exact remedy
(`aerial up --data-dir <X>`), and consider an `--autostart` that spawns the daemon on first
use.

### P2 — Lead with the hook model, not the manual loop
The manual `send → inbox → copy UUID → done` loop is race-prone and UUID-heavy. The
wake-driven, auto-acking modes (`watch --exec`, `agent exec`) are the correct pattern for
agents. Feature them first in the docs and de-emphasise manual `ack`.

### P2 — First-class Claude adapter
`aerial agent codex` already exists. A sibling `aerial agent claude` — or a documented
`aerial agent exec -- claude -p "$AERIAL_BODY"` — would let each inbound message spawn a
headless Claude Code run that reads and acks on success. Turnkey integration for Claude users.

### P3 — Structured payloads
The payload is `{ "body": string }`. For real agent work, allow a JSON payload and/or file
references (a task spec, a diff, an artifact path) rather than prose only.

---

## 3. Integration guidance (observed)

- For a single tool's **own** sub-agents, an in-process spawn+message primitive is simpler
  than a daemon. Aerial's differentiated value is **heterogeneous / persistent** meshes:
  e.g. a Claude Code agent ↔ a Codex agent, or long-lived daemon workers that wake on
  messages across terminals/machines.
- The clean persistent-worker pattern is `aerial agent exec -- <runner> …` that acks on
  success, so a supervisor never re-spawns cold workers.
- Aerial coordinates messaging but does **not** remove shared-resource contention (e.g. two
  workers both driving one browser). Serialise access to such resources at the task level.

---

_Tested against `aerial-local` 0.4.2 on Windows 11 via npm global install._
</content>
