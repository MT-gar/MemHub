<!-- name: Lessons learned -->
<!-- description: Distill reusable lessons from debugging notes, mistakes and things that worked -->
# MemHub summary task `{{task_id}}` — Lessons learned

You are consolidating memories collected from several AI agents (Claude Code, Codex, OpenClaw, …) into **reusable lessons**. The memories are below, each with an id like `[01J…]`.

## Rules

1. Only use what is in the entries. Do not invent facts. If something is unclear, say so.
2. Every lesson must cite the entries it comes from as `[src: <id>, <id>]`.
3. Prefer general, transferable lessons over one-off details. Merge duplicates across agents.
4. Mark lessons that look **outdated or contradictory** with `⚠️` and explain why.
5. Write in **{{language}}**. Output plain Markdown starting with a level-1 heading.

## Output format

```
# Lessons learned — {{date}}

## Top lessons
- **<short imperative lesson>** — why / when it applies. [src: …]
  (5–15 items, most valuable first)

## By area
### <area, e.g. build & tooling / debugging / APIs / process>
- … [src: …]

## Outdated or conflicting
- ⚠️ … [src: …]

## Suggested additions to shared instructions (AGENTS.md / CLAUDE.md)
- <rule phrased as an instruction an agent can follow>
```
