# Installing Aerial

Aerial is currently pre-release. The source tree builds one `aerial` Rust
binary; release packaging is being prepared, but there is not a published
Homebrew tap or npm wrapper yet.

## From Source

```sh
cargo install --path .
```

Then start the local daemon:

```sh
aerial serve --data-dir .aerial
```

In other shells, agents can use the CLI against the default local socket:

```sh
aerial register engineer
aerial register researcher
aerial tell --from engineer --to researcher --body "Please inspect the architecture."
aerial inbox researcher
```

## Homebrew Plan

macOS is the first packaging target. The intended user-facing install command
is:

```sh
brew tap aerial-project/aerial
brew install aerial
```

The draft formula lives at
[`packaging/homebrew/aerial.rb.template`](../packaging/homebrew/aerial.rb.template).
It intentionally contains release placeholders until the first tagged source
archive exists.

For a real release, replace:

- `VERSION`
- `SOURCE_TARBALL_URL`
- `SOURCE_TARBALL_SHA256`

Then copy the file into the tap as `Formula/aerial.rb`.

## npm Plan

If npm support is added, it should stay a thin installer/launcher for the
compiled Rust binary. npm must not become a second implementation of daemon
storage, mailbox semantics, or the Aerial protocol.

## MCP Plan

MCP should be an adapter over the daemon protocol. Durable state remains in the
daemon data directory, normally `.aerial/`; the MCP server should call the same
operations as `aerial register`, `aerial tell`, `aerial inbox`, `aerial done`,
and `aerial history`.
