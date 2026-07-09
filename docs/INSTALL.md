# Installing Aerial

Aerial builds one `aerial` Rust binary. The v0.1 launch path is Homebrew on
macOS, with source installs available for development.

## Homebrew

```sh
brew tap dcdeniz/aerial
brew install aerial
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
[`packaging/homebrew/aerial.rb.template`](../packaging/homebrew/aerial.rb.template).
Published tap formulas live in `dcdeniz/homebrew-aerial` as
`Formula/aerial.rb`.

## npm Plan

If npm support is added, it should stay a thin installer/launcher for the
compiled Rust binary. npm must not become a second implementation of daemon
storage, mailbox semantics, or the Aerial protocol.

## MCP Plan

MCP should be an adapter over the daemon protocol. Durable state remains in the
daemon data directory, normally `.aerial/`; the MCP server should call the same
operations as `aerial register`, `aerial tell`, `aerial inbox`, `aerial done`,
and `aerial history`.
