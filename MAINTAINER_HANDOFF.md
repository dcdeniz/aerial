# Aerial v0.2 — Handoff to the maintainer

Prepared by the two working agents (**claude** + **jeff**). Every issue #8–#13
has a PR; all code builds and tests pass on Linux/macOS/Windows (verified
locally on Windows). This note records only what we **can't** resolve
ourselves — merge mechanics, the release cut, and a couple of decisions that
are yours to make.

## PRs and merge order (they are STACKED — order matters)

All PRs are from the fork `sguckiran/aerial` into `dcdeniz/aerial:main`.

| PR | Issue | What | Base (stacked on) |
|----|-------|------|-------------------|
| #17 | — | Windows transport (AF_UNIX via `uds_windows`) + CI | main |
| #18 | #8, #11 | daemon wake notifications + `aerial watch` | #17 |
| #20 | #9 | `aerial watch --exec` hook | #18 |
| #21 | #12 (wake) | watch/exec docs + wake integration tests | #20 |
| #19 | #10 | MCP adapter (`aerial mcp`, stdio) | #17 |
| #22 | #12 (MCP) | MCP docs (`docs/MCP.md`) + tests | #19 |
| #23 | #13 (A) | bump 0.2.0 + `CHANGELOG.md` | #21 |
| #25 | #13 (B) | Homebrew formula + `INSTALL.md` for v0.2 | #22 |
| #24 | — | `AGENTS.md` inbox rule + command reference | #17 |

**Merge #17 first.** Then chain A: #18 → #20 → #21 → #23. Then chain B:
#19 → #22 → #25. #24 any time after #17. Merge the feature PRs **before**
#23/#24/#25 so the docs don't reference commands not yet in the tree.

### Expected conflicts — resolve by KEEPING BOTH sides (none are semantic)
- `src/main.rs`: both chains add a subcommand + match arm after `History`
  (chain A `Watch`, chain B `Mcp`).
- `src/lib.rs`: chain A adds a `WatchEvent` re-export; chain B adds
  `pub mod mcp;` (different lines — usually auto-merges).
- `README.md`: chain A adds a "Waking agents" section, chain B an "MCP"
  section (both just after Quickstart).

## #13 — the actual release cut is YOURS (we could only prep it)

Neither agent can tag or publish from a fork, and #13 is gated on the feature
work being merged. After merging the stack:
1. Version + changelog are already in **#23** (`Cargo.toml`/`Cargo.lock` →
   `0.2.0`, `CHANGELOG.md`).
2. `git tag v0.2.0` and publish the release source tarball.
3. Compute the tarball `sha256`.
4. Fill `packaging/homebrew/aerial-local.rb.template`
   (`VERSION` / `SOURCE_TARBALL_URL` / `SOURCE_TARBALL_SHA256`) and copy it to
   `dcdeniz/homebrew-aerial` as `Formula/aerial-local.rb` (**#25** left these
   as publish-time placeholders on purpose).

## Open decisions for you

1. **Exec hook (#9) has no automated test.** The feature works — manually
   verified on Windows: the hook fires on arrival with `AERIAL_AGENT`,
   `AERIAL_MESSAGE_ID`, `AERIAL_SOCKET` set and the id matching the delivered
   envelope — and it's documented, but there is no test in `tests/`. Every
   other feature is tested. **Decision: accept for v0.2, or add a
   `watch --exec` integration test first?** (Easy to add; we left it out
   rather than ship a flaky cross-platform subprocess test without your call.)

2. **Wake stale-watcher cleanup (known limitation, noted in #18).** A watcher
   whose client disconnects while no new mail arrives is not pruned until the
   next `send` to that agent (its thread parks on the channel). Harmless for
   small local swarms; a periodic heartbeat/ping would tighten it. **Accept for
   v0.2, or fix before release?**

3. **Delivery is at-least-once** (per `ARCHITECTURE.md`), so MCP/exec handlers
   must be idempotent — worth a line in release notes if you want it explicit.
   The pre-existing `ARCHITECTURE.md` open questions (exactly-once delivery,
   registry persistence across daemon restarts, wire-format versioning) are
   untouched by this work and remain open.

## Verification already done (so you don't have to re-run it)
- Built + tested **both** chains on Windows: chain A = 11 tests (9 unit + 2
  wake integration); chain B = 21 tests. All green.
- Inspected `src/mcp.rs` directly: no separate mailbox state, every tool
  dispatched via `daemon::request`, exactly the five tools `register`, `tell`,
  `inbox`, `done`, `history`.
- #25 reviewed for conflicts against chain A: none.
