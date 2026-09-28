<!-- name: Deduplicate & reconcile -->
<!-- description: Find duplicated, stale or contradicting memories across agents and propose merges -->
# MemHub summary task `{{task_id}}` — Deduplicate & reconcile

Review memories from several agents and find **duplicates, stale facts and contradictions**. Propose concrete merges. Do not rewrite the memories yourself — produce a review the user can act on.

## Rules

1. Group entries that say the same thing (across agents and projects). Cite all ids.
2. For contradictions, quote the conflicting statements briefly and say which one is likely current (use `updated` dates).
3. Flag entries that look obsolete (refer to removed tools, old versions, finished tasks).
4. Write in **{{language}}**. Output plain Markdown starting with a level-1 heading.

## Output format

```
# Memory review — {{date}}

## Duplicates (merge candidates)
### <topic>
- Entries: [src: …] — proposed canonical wording: "…" — keep in: <agent/file>

## Contradictions
- ⚠️ <topic>: "<A>" [src] vs "<B>" [src] → recommendation

## Probably obsolete
- [src: …] — why

## Summary
- <n> duplicate groups, <n> contradictions, <n> obsolete entries
```
