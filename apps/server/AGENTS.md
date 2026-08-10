# Agent Guide

## Repository summary

- Rust 1.97.1, edition 2024, axum, Tokio, SQLx SQLite, and code-first OpenAPI 3.1.
- `CONTRACT.md` is the public compatibility boundary shared with the Go backend and both frontends.
- Runtime settings come from `config/config.yaml` and optional `config/config.local.yaml`.

## Commands

- Install: `mise install`
- Format: `mise exec -- cargo fmt --all`
- Format check: `mise exec -- cargo fmt --all -- --check`
- Lint: `mise exec -- cargo clippy --all-targets --all-features -- -D warnings`
- Test: `mise exec -- cargo test`
- Build: `mise exec -- cargo build --release --locked`
- Run: `mise exec -- cargo run`

## Rules

- Preserve contract paths, status codes, JSON field names, and Problem semantics.
- Keep route, auth, configuration, database, state, and error responsibilities separated.
- Unsafe code is forbidden.
- Use Simplified Chinese comments only for non-obvious project-owned logic.
- Never log JWTs, passwords, password hashes, or configuration secrets.
- Add tests when changing configuration merging, authentication, routes, or OpenAPI output.
- Do not commit local configuration, SQLite data, `target`, or generated runtime files.
