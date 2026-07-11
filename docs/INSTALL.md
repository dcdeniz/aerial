# Installing Aerial

Aerial builds one `aerial` Rust binary. Homebrew on macOS is the primary
install path, with source installs available for development. As of v0.2 the
daemon transport also builds and runs on Windows (via AF_UNIX sockets)
alongside macOS and Linux.

## Homebrew

```sh
brew tap dcdeniz/aerial
brew trust dcdeniz/aerial
brew install dcdeniz/aerial/aerial-local
```

Then start the local daemon:

```sh
aerial up
```

In other shells, agents can use the CLI against the default local socket:

```sh
aerial join engineer
aerial join researcher
aerial send --from engineer --to researcher --body "Please inspect the architecture."
aerial read researcher
```

## From Source

```sh
cargo install --path .
```

Then start the local daemon:

```sh
aerial up
```

In other shells, agents can use the CLI against the default local socket:

```sh
aerial join engineer
aerial join researcher
aerial send --from engineer --to researcher --body "Please inspect the architecture."
aerial read researcher
```

The formula template lives at
[`packaging/homebrew/aerial-local.rb.template`](../packaging/homebrew/aerial-local.rb.template).
Published tap formulas live in `dcdeniz/homebrew-aerial` as
`Formula/aerial-local.rb`.

## npm Plan

If npm support is added, it should stay a thin installer/launcher for the
compiled Rust binary. npm must not become a second implementation of daemon
storage, mailbox semantics, or the Aerial protocol.

## MCP

Aerial ships an MCP adapter over the daemon protocol. The hidden `aerial mcp`
subcommand speaks JSON-RPC 2.0 over stdio and exposes five tools — `register`,
`tell`, `inbox`, `done`, and `history` — each dispatched to the running daemon.
It keeps no separate mailbox state: durable state stays in the daemon data
directory, normally `.aerial/`.

```sh
aerial mcp --socket .aerial/aerial.sock
```

See [`MCP.md`](MCP.md) for the tool reference, MCP client configuration, and an
example stdio session.
