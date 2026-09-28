# Changelog

## Unreleased (M0 scaffold)
- `MEMHUB_USER_HOME` overrides the user home scanned by adapters (Windows ignores `HOME`); watcher ignores `.git` on Windows paths too; Windows/MSVC build notes.
- Core library: adapters (Claude Code, Codex CLI, Gemini CLI, OpenClaw, Windsurf, generic folders, project files), Markdown vault with frontmatter, SQLite FTS5 index, secret redaction, git snapshots, file watcher.
- Summary tasks with five built-in templates; bring-your-own-agent execution via MCP, CLI or copy-paste; accept results as knowledge notes.
- MCP server (stdio) with 7 tools; `memhub` CLI (scan / watch / mcp / serve / …); browser mode with embedded UI.
- Web UI (React + Vite, zh-CN / en) shared by the Tauri desktop app and browser mode.
- Tauri 2 desktop shell, icons, GitHub Actions CI and multi-platform release workflow.
