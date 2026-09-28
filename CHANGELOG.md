# Changelog

All notable changes to MemHub are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow SemVer.

## [Unreleased] — first public release (0.1.0)

### Added
- Core library: adapters (Claude Code, Codex CLI, Gemini CLI, OpenClaw, Windsurf, generic folders, project files), Markdown vault with frontmatter, SQLite FTS5 index, secret redaction, git snapshots, file watcher.
- Summary tasks with five built-in templates (lessons, preferences, project brief, dedupe, digest); bring-your-own-agent execution via MCP, CLI or copy-paste; accept results as knowledge notes with source traceability.
- MCP server (stdio) with 7 tools; `memhub` CLI (`scan` / `watch` / `mcp` / `serve` / `detect` / `list` / `search` / `show` / `paths` / `reindex`); browser mode with the UI embedded in the binary.
- Web UI (React + Vite, zh-CN / en) shared by the Tauri desktop app and browser mode: overview, memories, knowledge, tasks, sources, settings.
- Light / dark / system theme (Settings → Appearance, `?theme=` URL override, persisted in `localStorage`).
- Hash-based deep links: `#/memories/<id>`, `#/knowledge/<id>`, `#/tasks/<id>`, `#/sources`, `#/settings`; `?lang=en|zh-CN` overrides the configured UI language.
- Tauri 2 desktop shell with the CLI as a sidecar; app icons; Windows MSI / NSIS bundles verified.
- Windows support: `MEMHUB_USER_HOME` overrides the user home scanned by adapters (Windows ignores `HOME`); mixed path separators normalised; the Tasks page generates PowerShell commands (`claude -p (Get-Content -Raw 'TASK.md') | Out-File -Encoding utf8 'result.md'`) for Windows task directories; `result.md` files starting with a UTF-8 BOM are accepted.
- GitHub project files: CI matrix (Ubuntu + Windows, desktop shell `cargo check`), release workflow for `v*` tags (desktop bundles for Windows / macOS / Linux + standalone CLI archives), issue / PR templates, `CONTRIBUTING.md`, `SECURITY.md`, `.editorconfig`, `.gitattributes`.
- Illustrated README (EN / 中文) with screenshots, architecture diagram and configuration reference; `docs/DESIGN.md`; Windows walkthrough in `docs/mcp-memhub-bootstrap.md`.

### Changed
- Dates in the UI are rendered as `YYYY-MM-DD HH:mm` regardless of the browser locale.
- `project_path` in the entry detail view is localised.
