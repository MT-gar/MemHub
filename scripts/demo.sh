#!/usr/bin/env bash
# Creates a fake HOME with memory files of several agents, for demos / screenshots / dev.
# Usage:  ./scripts/demo.sh [target-dir]      (default: ./demo/home)
#         HOME=$PWD/demo/home MEMHUB_USER_HOME=$PWD/demo/home MEMHUB_HOME=$PWD/demo/home/.memhub memhub serve
set -euo pipefail
H="${1:-$(pwd)/demo/home}"
rm -rf "$H"
mkdir -p "$H"

# ---------- Claude Code ----------
mkdir -p "$H/.claude/projects/-home-user-dev-shop-api/memory" \
         "$H/.claude/projects/-home-user-dev-blog/memory" \
         "$H/.claude/agent-memory/code-reviewer"
cat > "$H/.claude/CLAUDE.md" <<'EOF'
# Global preferences

- Reply in Chinese unless the code comments require English.
- Prefer pnpm over npm; never commit lockfiles from other package managers.
- Always run the test suite before proposing a commit.
- Keep commit messages in Conventional Commits format.
EOF
cat > "$H/.claude/projects/-home-user-dev-shop-api/memory/MEMORY.md" <<'EOF'
# shop-api memory

## Build & run
- `make dev` starts Postgres + API via docker compose; port 8080.
- Migrations live in `db/migrations`, run with `make migrate` (uses golang-migrate).

## Conventions
- Handlers return `apiError` structs; never `panic` in request path.
- Money is stored as integer cents (`amount_cents`), never floats.

## Lessons
- The flaky `TestCheckout` failure was a timezone issue: CI runs in UTC, the fixture assumed Asia/Singapore. See debugging.md.
- Stripe webhooks must be idempotent — we key on `event.id` in `webhook_events` table.
EOF
cat > "$H/.claude/projects/-home-user-dev-shop-api/memory/debugging.md" <<'EOF'
# Debugging notes

## 2026-09-12 flaky TestCheckout
Symptom: passes locally, fails on CI with `expected 2026-09-12 got 2026-09-11`.
Root cause: `time.Now()` in fixture, CI TZ=UTC. Fix: inject clock, set TZ in test setup.
Lesson: never depend on wall-clock in tests; always inject a clock.

## 2026-09-20 connection pool exhaustion
`pgx` pool max was 4 under load tests. Raised to 20 and added `defer rows.Close()` in `ListOrders`.
EOF
cat > "$H/.claude/projects/-home-user-dev-shop-api/memory/api-conventions.md" <<'EOF'
# API conventions
- Versioned under `/v1`; breaking changes → `/v2`, never mutate v1 semantics.
- Pagination: cursor-based (`?cursor=`), page size max 100.
- Errors: `{ "error": { "code": "...", "message": "..." } }`.
EOF
cat > "$H/.claude/projects/-home-user-dev-shop-api/9f1c2b7e-session.jsonl" <<'EOF'
{"type":"user","cwd":"/home/user/dev/shop-api","sessionId":"9f1c2b7e","message":{"role":"user","content":"fix the flaky checkout test"}}
EOF
cat > "$H/.claude/projects/-home-user-dev-blog/memory/MEMORY.md" <<'EOF'
# blog memory
- Astro 5 site, deployed to Cloudflare Pages on push to `main`.
- Images must go through `astro:assets`; raw `<img>` breaks the build.
- The user likes short paragraphs and dislikes emoji in headings.
EOF
cat > "$H/.claude/projects/-home-user-dev-blog/a1b2-session.jsonl" <<'EOF'
{"type":"user","cwd":"/home/user/dev/blog","message":{"role":"user","content":"new post"}}
EOF
cat > "$H/.claude/agent-memory/code-reviewer/MEMORY.md" <<'EOF'
# code-reviewer memory
- This user wants review comments grouped by severity (blocker / should-fix / nit).
- Common miss: forgetting to close `rows` in Go database code.
EOF

# ---------- Codex CLI ----------
mkdir -p "$H/.codex/memories/rollout_summaries" "$H/.codex/memories/skills/deploy"
cat > "$H/.codex/AGENTS.md" <<'EOF'
# Codex global instructions
- Ask before running destructive git commands.
- Use `rg` instead of `grep`.
EOF
cat > "$H/.codex/memories/memory_summary.md" <<'EOF'
# Memory summary
- User works on shop-api (Go) and blog (Astro). Prefers small PRs.
- Deploy of shop-api is `make deploy ENV=staging`; production needs a tag `v*`.
- OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz0123456789 was pasted once; do not echo it.
EOF
cat > "$H/.codex/memories/MEMORY.md" <<'EOF'
# Codex long-form memory
## shop-api
- Integration tests need `docker compose up -d db` first, otherwise they hang for 60s.
- `golangci-lint` config forbids `fmt.Println` in non-test code.
## blog
- Netlify was replaced by Cloudflare Pages in August 2026.
EOF
cat > "$H/.codex/memories/rollout_summaries/2026-09-20-shop-api.md" <<'EOF'
# Session 2026-09-20 — shop-api load test
Raised pgx pool size, added row closing, wrote a k6 script under `loadtest/`. Open: p95 still 420 ms on /v1/orders.
EOF
cat > "$H/.codex/memories/skills/deploy/SKILL.md" <<'EOF'
# Skill: deploy shop-api
1. `make test` 2. `git tag vX.Y.Z && git push --tags` 3. watch GitHub Actions "release" workflow.
EOF

# ---------- Gemini CLI ----------
mkdir -p "$H/.gemini"
cat > "$H/.gemini/GEMINI.md" <<'EOF'
# Gemini context
- The user's timezone is Asia/Singapore.
- For data analysis tasks prefer polars over pandas.
EOF

# ---------- OpenClaw ----------
mkdir -p "$H/.openclaw/workspace/memory"
cat > "$H/.openclaw/workspace/MEMORY.md" <<'EOF'
# Long-term memory
- Owner: Alex. Prefers concise replies, no emoji.
- Weekly review every Sunday 20:00; remind on Telegram.
- Home NAS at 192.168.1.20 runs the backups; check `zpool status` if alerts fire.
EOF
cat > "$H/.openclaw/workspace/USER.md" <<'EOF'
# USER
Name: Alex. Developer, based in Singapore. Working on shop-api and a personal blog.
EOF
cat > "$H/.openclaw/workspace/SOUL.md" <<'EOF'
# SOUL
Be direct, practical, slightly dry humour. Never post secrets.
EOF
cat > "$H/.openclaw/workspace/memory/2026-09-26.md" <<'EOF'
# 2026-09-26
- Booked dentist for Oct 3, 10:00.
- Alex asked to summarise the shop-api incident for the team; draft saved in Notes.
- Learned: Alex hates being asked "anything else?" at the end of a reply.
EOF
cat > "$H/.openclaw/workspace/memory/2026-09-27.md" <<'EOF'
# 2026-09-27
- NAS backup alert at 03:12 — false positive after the ZFS scrub; documented in projects.md.
- Alex wants a weekly digest of what all coding agents did. (→ this is why MemHub exists)
EOF
cat > "$H/.openclaw/workspace/memory/projects.md" <<'EOF'
# Projects
- shop-api: Go, staging on Fly.io, prod on Hetzner. On-call rotation doc in Notion.
- blog: Astro on Cloudflare Pages.
- NAS: TrueNAS, monthly scrub on the 27th (alerts during scrub are expected).
EOF

# ---------- A registered project directory ----------
mkdir -p "$H/dev/shop-api/.cursor/rules" "$H/dev/shop-api/db/migrations"
cat > "$H/dev/shop-api/CLAUDE.md" <<'EOF'
# shop-api — instructions for Claude Code
- Run `make test` before every commit. Never skip the race detector.
- Money is integer cents. Timestamps are UTC in the DB, converted at the edge.
EOF
cat > "$H/dev/shop-api/AGENTS.md" <<'EOF'
# shop-api — AGENTS.md
- Go 1.24, chi router, pgx. Lint with golangci-lint.
- Do not add new dependencies without asking.
EOF
cat > "$H/dev/shop-api/.cursor/rules/style.mdc" <<'EOF'
---
description: Go style rules
globs: ["**/*.go"]
---
- Wrap errors with %w and context; no naked returns.
EOF

# ---------- MemHub config pointing at the project ----------
# Under Git Bash / MSYS the config must hold a Windows path (C:/...), not /c/...
HW="$H"
if command -v cygpath >/dev/null 2>&1; then HW="$(cygpath -m "$H")"; fi
mkdir -p "$H/.memhub"
cat > "$H/.memhub/config.toml" <<EOF
vault = "~/.memhub/vault"
language = "zh-CN"
git_snapshot = false
max_file_size_kb = 2048
redact_secrets = true
task_context_budget_kb = 200

[[sources]]
type = "claude-code"
[[sources]]
type = "codex"
[[sources]]
type = "gemini"
[[sources]]
type = "openclaw"
[[sources]]
type = "windsurf"

[[projects]]
path = "$HW/dev/shop-api"
name = "shop-api"
EOF

echo "Demo home created at $H"
echo "Run:  HOME=$H MEMHUB_USER_HOME=$H MEMHUB_HOME=$H/.memhub memhub serve"
