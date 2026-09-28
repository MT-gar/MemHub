# Contributing to MemHub

Thanks for helping! MemHub is a small Rust + React code base; most contributions fall into one of these buckets.

## Adding an agent adapter

1. Find out where the agent stores its memory / instruction files (paths, formats, what is user-authored vs. tool-managed).
2. Add `crates/memhub-core/src/adapters/<agent>.rs` implementing `collect()` (see `windsurf.rs` for the smallest example, `openclaw.rs` for a multi-workspace one) and register it in `adapters/mod.rs`.
3. Add fixture files to `scripts/demo.sh` so the demo home exercises the adapter.
4. Add the agent to the "Supported agents" table in `README.md` / `README.zh-CN.md` and a line to `CHANGELOG.md`.

## Adding a summary template

Templates live in `crates/memhub-core/templates/*.md` and are copied to `~/.memhub/templates/` on first run (user copies win). Front matter declares the title/description; the body is the prompt with `{{...}}` placeholders (see `lessons.md`).

## Development loop

```bash
./scripts/demo.sh                 # fake home with memories from several agents
./scripts/dev.sh                  # API on :7337 (demo data) + Vite HMR on :1420
cargo test --workspace
npm run dev --prefix apps/desktop # desktop window (needs the Tauri prerequisites)
```

Windows: use the MSVC toolchain (`rustup override set stable-x86_64-pc-windows-msvc` inside the repo) and run the scripts from Git Bash. See `docs/mcp-memhub-bootstrap.md` for a full Windows walkthrough.

## Ground rules

- Never commit vault contents, `~/.memhub`, or real agent memory files — the demo fixtures are the only "memories" that belong in the repo.
- Keep the app local-first: no network calls, no telemetry, no bundled LLM.
- Formatting: `cargo fmt`, 2-space indentation for TS/JSON/YAML (see `.editorconfig`).
