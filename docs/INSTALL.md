# Installing Aerial

Aerial builds one `aerial` Rust binary. npm on 64-bit Windows and Homebrew on
macOS are the supported package-manager install paths, with source installs
available for development. As of v0.4,
Aerial includes the local daemon, MCP adapter, wake notifications, CLI/MCP flow
macros, and an agent supervisor. The daemon transport also builds and runs on
Windows (via AF_UNIX sockets) alongside macOS and Linux.

## npm on Windows

The `aerial-local` npm package bundles the compiled Windows executable. npm is
only the installer and launcher: daemon storage, mailbox semantics, and the
protocol remain implemented by the Rust binary.

```powershell
npm install --global aerial-local
aerial --version
aerial up
```

The npm package currently supports 64-bit Windows (`win32-x64`). It does not
download an executable during installation, so installs remain reproducible
and do not depend on lifecycle scripts.

For agents launched from different working directories, set `AERIAL_SOCKET` to
the same absolute socket path in every process. `AERIAL_DATA_DIR` provides the
matching default for `aerial up`.

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

To let an agent wake and work without manual prompting, run a supervisor:

```sh
aerial agent codex researcher --cd .
```

## Local flow macros

After installation, start the local daemon:

```sh
aerial up
```

In other shells, agents can use one-command flow macros against the default
local socket:

```sh
aerial exchange --from engineer --to researcher --body "Please inspect the architecture."
aerial status researcher
aerial drain researcher
```

To let an agent wake and work without manual prompting, run a supervisor:

```sh
aerial agent codex researcher --cd .
```

The formula template lives at
[`packaging/homebrew/aerial-local.rb.template`](../packaging/homebrew/aerial-local.rb.template).
Published tap formulas live in `dcdeniz/homebrew-aerial` as
`Formula/aerial-local.rb`.

## npm release

The package source lives under [`npm/`](../npm/). The npm workflow compiles the
locked Rust project on Windows, copies `aerial.exe` into the package, runs the
installed-style smoke test, and publishes releases from GitHub Actions. The
version in `npm/package.json` must match `Cargo.toml` and the release tag.

## MCP

Aerial ships an MCP adapter over the daemon protocol. The hidden `aerial mcp`
subcommand speaks JSON-RPC 2.0 over stdio and exposes primitive tools
(`register`, `tell`, `inbox`, `done`, `history`) plus flow macros (`status`,
`drain`, `exchange`). Each call is dispatched to the running daemon. It keeps no
separate mailbox state: durable state stays in the daemon data directory,
normally `.aerial/`.

```sh
aerial mcp --socket .aerial/aerial.sock
```

See [`MCP.md`](MCP.md) for the tool reference, MCP client configuration, and an
example stdio session.
