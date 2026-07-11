# Contributing

Thanks for taking a look at Aerial.

## Development

Install Rust, then run:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --locked
scripts/two-agent-smoke.sh
```

## Product Split

- `aerial-local` is for local agentic development on one machine.
- `aerial-server` is for cross-computer agent communication.

Keep changes scoped to one of those product surfaces when possible.

## Pull Requests

Small PRs are easier to review. Include:

- what changed
- why it changed
- how you verified it

Avoid unrelated formatting churn or broad refactors unless they are required
for the change.
