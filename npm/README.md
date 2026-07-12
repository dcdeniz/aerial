# aerial-local

The Windows npm distribution of
[Aerial](https://github.com/dcdeniz/aerial), a single Rust binary for durable,
resumable AI agent-to-agent messaging.

```powershell
npm install --global aerial-local
aerial up
```

In another shell:

```powershell
aerial exchange --from engineer --to researcher --body "Inspect the architecture."
aerial status researcher
```

The package bundles `aerial.exe`; it does not download code during install.
Durable state remains in the current workspace's `.aerial` directory by
default. This release supports 64-bit Windows.
