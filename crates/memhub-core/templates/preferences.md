<!-- name: Preferences & conventions -->
<!-- description: Extract the user's stable preferences, coding conventions and no-gos across all agents -->
# MemHub summary task `{{task_id}}` — Preferences & conventions

Extract the **user's durable preferences** from memories written by several AI agents, so that every agent can follow the same conventions.

## Rules

1. Only include preferences supported by the entries; cite them as `[src: <id>]`.
2. Distinguish **global** preferences (apply everywhere) from **project-specific** ones.
3. Merge duplicates, resolve wording differences, flag contradictions with `⚠️`.
4. Phrase each item as a short instruction an agent can follow.
5. Write in **{{language}}**. Output plain Markdown starting with a level-1 heading.

## Output format

```
# Preferences & conventions — {{date}}

## Global (paste into any AGENTS.md / CLAUDE.md / GEMINI.md)
- Communication: …
- Coding style: …
- Tools & workflow: …
- Never do: …

## Per project
### <project>
- … [src: …]

## Contradictions to resolve
- ⚠️ … [src: …]
```
