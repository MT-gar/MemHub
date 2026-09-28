<!-- name: Project brief -->
<!-- description: A knowledge card per project: architecture, decisions, conventions, commands, open issues -->
# MemHub summary task `{{task_id}}` — Project brief

Produce a **knowledge card for each project** that appears in the entries, so any agent can be onboarded in one read.

## Rules

1. One section per project (`project` field). Skip `_global` unless it has project-specific content.
2. Only use facts from the entries; cite `[src: <id>]`. Do not guess at architecture.
3. Keep each card under ~40 lines. Prefer commands, paths and decisions over prose.
4. Write in **{{language}}**. Output plain Markdown starting with a level-1 heading.

## Output format

```
# Project briefs — {{date}}

## <project name>  (<project_path if known>)
**What it is:** …
**Stack & architecture:** …
**Key decisions:** … [src: …]
**Conventions:** …
**Useful commands / paths:** …
**Known pitfalls:** … [src: …]
**Open questions / TODO:** …
```
