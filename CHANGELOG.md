# Changelog

All notable changes to MemHub are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer.

## [Unreleased]

### Added
- **Rules (pull-based feedback to agents).** New entry kind `rule` stored under `vault/rules/<scope>/` with a `draft → approved → retired` lifecycle; only approved rules are served. Scopes: `global` (personal preferences) and `project:<name>`.
- MCP tools `rules_get` (approved rules as compact Markdown, budgeted to 40 rules / 4 KiB, global first) and `rule_propose` (propose-only: creates a draft, cannot approve).
- CLI `memhub context [--project PATH|NAME] [--global] [--format md|json] [--max-lines N] [--with-ids]` (prints nothing when nothing is approved) and `memhub rules list|add|approve|retire|draft|rm`; approving needs an interactive terminal or `--yes`.
- **Rules** page in the app: review queue with a badge, per-rule approve / retire / edit / delete, scope filter, live preview of exactly what agents receive, copy-ready one-line instruction for `AGENTS.md`. The Overview shows the number of drafts waiting.
- `rules` summary template; accepting such a task turns its bullets into **draft** rules (with `[src: id]` evidence) — never into active ones.
- HTTP / Tauri API: `list_rules`, `add_rule`, `set_rule_status`, `edit_rule`, `preview_rules`.
- Guard rails: one line ≤ 300 chars, secrets and hidden/bidirectional characters refused, duplicates folded, review hints for shell commands / URLs / override attempts, at most 50 pending drafts, rules excluded from summary tasks.

## [0.1.1] — 2026-10-01

Stability release: first validation on a real machine, hardened browser mode, more tests.

### Security
- `memhub serve` now validates every request. Loopback binds only accept a loopback `Host` (DNS-rebinding protection); an `Origin` header, when present, must be the server itself or an `--allow-origin` (cross-site requests get 403, `Origin: null` included); `POST /api/*` must be `application/json` (no preflight-free form posts). Binding to a non-loopback address — or passing `--token` / `MEMHUB_TOKEN` — requires an access token (auto-generated, 192-bit) sent as `Authorization: Bearer`, `X-MemHub-Token` or an `HttpOnly; SameSite=Strict` cookie; open the printed `http://…/?token=…` link once. `/api/health` stays open.

### Fixed
- A fresh install no longer starts with every built-in source disabled: Claude Code, Codex, Gemini CLI, OpenClaw and Windsurf are enabled on first run (agents that are not installed are skipped as before).
- Claude Code project directories without session transcripts were named after the last dash-separated fragment of the encoded path (`C`, `C-2026`, `skill`) and had no `project_path`. The real path is now recovered by matching the encoded name against the filesystem (handles CJK characters, `_`, `.`, spaces and dashes); otherwise a readable slug is used. Projects that were mirrored under the old names are re-mirrored under the new ones; the old entries are archived, not deleted.
- File-watcher debounce branch that did the same thing twice; four `clippy` findings on current stable.

### Changed
- Tests: 6 → 35 (adapter fixtures for Claude Code / Codex / OpenClaw / project files / generic folders, sync update + size limit, CJK search, request-guard unit tests and router tests). The end-to-end test no longer depends on the machine's real agent directories.
- CI: macOS added to the test matrix, `--locked` builds, `clippy --all-targets -D warnings` is now a hard gate on Linux.
- New CLI flags: `memhub serve --allow-origin <ORIGIN>` (repeatable) and `--token <TOKEN>`.
- Docs: real-machine verification report (`docs/mcp-real-machine-verification.md`), next-steps analysis (`docs/mcp-next-steps.md`).

## [0.1.0] — 2026-09-28

First public release.

### Added
- Core library: adapters (Claude Code, Codex CLI, Gemini CLI, OpenClaw, Windsurf, generic folders, project files), Markdown vault with frontmatter, SQLite FTS5 index, secret redaction, git snapshots, file watcher.
- Summary tasks with five built-in templates (lessons, preferences, project brief, dedupe, digest); bring-your-own-agent execution via MCP, CLI or copy-paste; accept results as knowledge notes with source traceability.
- MCP server (stdio) with 7 tools; `memhub` CLI (`scan` / `watch` / `mcp` / `serve` / `detect` / `list` / `search` / `show` / `paths` / `reindex`); browser mode with the UI embedded in the binary.
- Web UI (React + Vite, zh-CN / en) shared by the Tauri desktop app and browser mode: overview, memories, knowledge, tasks, sources, settings.
- Light / dark / system theme (Settings → Appearance, `?theme=` URL override, persisted in `localStorage`).
- Hash-based deep links: `#/memories/<id>`, `#/knowledge/<id>`, `#/tasks/<id>`, `#/sources`, `#/settings`; `?lang=en|zh-CN` overrides the configured UI language.
- Tauri 2 desktop shell with the CLI as a sidecar; app icons; Windows MSI / NSIS bundles verified.
- Windows support: `MEMHUB_USER_HOME` overrides the user home scanned by adapters (Windows ignores `HOME`); mixed path separators normalised; the Tasks page generates PowerShell commands (`claude -p (Get-Content -Raw 'TASK.md') | Out-File -Encoding utf8 'result.md'`) for Windows task directories; `result.md` files starting with a UTF-8 BOM are accepted.
- GitHub project files: CI matrix (Ubuntu + Windows, desktop shell `cargo check`), release workflow for `v*` tags (draft release with desktop bundles for Windows / macOS / Linux + standalone CLI archives, notes taken from this file), issue / PR templates, `CONTRIBUTING.md`, `SECURITY.md`, `.editorconfig`, `.gitattributes`.
- Illustrated README (EN / 中文) with screenshots, architecture diagram and configuration reference; `docs/DESIGN.md`; Windows walkthrough in `docs/mcp-memhub-bootstrap.md`.

### Changed
- Dates in the UI are rendered as `YYYY-MM-DD HH:mm` regardless of the browser locale.
- `project_path` in the entry detail view is localised.

[Unreleased]: https://github.com/MT-gar/MemHub/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/MT-gar/MemHub/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/MT-gar/MemHub/releases/tag/v0.1.0
