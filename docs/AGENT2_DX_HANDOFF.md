# Agent 2 DX / Packaging Handoff

Context: agent 1 has started the Rust implementation as a single `aerial`
binary with a local daemon and mailbox protocol. The current CLI surface is:

- `aerial serve --data-dir .aerial`
- `aerial register <name>`
- `aerial tell --from <agent> --to <agent> --body <message>`
- `aerial inbox <agent>`
- `aerial done --agent <agent> <envelope-id>`
- `aerial history [--limit N] [--json]`

The user wants Aerial to become a single installable package, ideally via
Homebrew or npm, that agents can use to communicate through durable mailboxes.
They also want either a daemon, an MCP server, or both. The daemon exists first;
MCP should be treated as an adapter over the same daemon protocol, not a second
source of truth.

Recommended Agent 2 work:

1. Packaging DX
   - Add a release-friendly install story for macOS first.
   - Prefer Homebrew formula support for the Rust binary.
   - Consider an npm package only as a thin wrapper that downloads/runs the
     compiled binary; do not reimplement Aerial in Node.
   - Add a short `README.md` quickstart once the command names settle.

2. MCP Adapter
   - Expose MCP tools that map directly to daemon requests:
     `register`, `tell`, `inbox`, `done`, and `history`.
   - Keep durable state in the daemon's `.aerial/` data directory.
   - Do not write separate MCP mailbox files.

3. UX Details
   - Make `aerial history` useful for humans first:
     `Agent <short-id> -> Agent <short-id> "Message First 50 Characters ...."`.
   - Preserve `--json` modes for agent/tool consumers.
   - Keep defaults local and infrastructure-free.

Coordination notes:

- Source of truth for storage is the Rust daemon and JSONL mailbox/transcript
  files.
- Avoid redesigning `site/index.html` unless the user explicitly asks to change
  the retro deck bit.
- If adding docs, keep them honest about current implementation status: daemon
  request/response works; live push delivery and MCP are not implemented yet.
