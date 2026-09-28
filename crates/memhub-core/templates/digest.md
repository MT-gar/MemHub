<!-- name: Periodic digest -->
<!-- description: What each agent worked on and learned in the period; pending items -->
# MemHub summary task `{{task_id}}` — Periodic digest

Write a **digest of the selected period**: what the agents worked on, what was learned, and what is still open.

## Rules

1. Organise by agent, then by project. Use the `updated` dates to order events.
2. Facts only, cited as `[src: <id>]`. Keep it scannable (bullets, ≤ 2 lines each).
3. End with a short "carry forward" list of unfinished items and decisions to make.
4. Write in **{{language}}**. Output plain Markdown starting with a level-1 heading.

## Output format

```
# Digest — {{date}}

## Highlights
- …

## By agent
### <agent>
#### <project>
- <date>: … [src: …]

## Learned
- …

## Carry forward
- [ ] …
```
