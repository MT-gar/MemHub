//! Summary tasks: "bring your own agent" distillation.
//!
//! A task is a directory under `~/.memhub/tasks/<id>/` holding:
//! - `TASK.md`   the full prompt (template + scope + inlined memory entries)
//! - `task.json` metadata / status
//! - `result.md` what the agent produced (via MCP `summary_task_submit`, or pasted in the GUI)
//!
//! Accepting a task copies the result into `vault/knowledge/` as a knowledge note.

use crate::hub::{now, Hub};
use crate::model::{EntryFilter, EntrySummary, Kind};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const BUILTIN_TEMPLATES: &[(&str, &str)] = &[
    ("lessons", include_str!("../templates/lessons.md")),
    ("preferences", include_str!("../templates/preferences.md")),
    ("project-brief", include_str!("../templates/project-brief.md")),
    ("dedupe", include_str!("../templates/dedupe.md")),
    ("digest", include_str!("../templates/digest.md")),
    ("_footer", include_str!("../templates/_footer.md")),
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskScope {
    pub agents: Vec<String>,
    pub projects: Vec<String>,
    pub kinds: Vec<String>,
    pub query: Option<String>,
    /// RFC3339 or `YYYY-MM-DD` lower bound on `updated`.
    pub since: Option<String>,
    pub limit: Option<usize>,
    pub include_knowledge: bool,
    pub include_archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMeta {
    pub id: String,
    pub template: String,
    pub title: String,
    /// pending → done (result submitted) → accepted (knowledge note written)
    pub status: String,
    pub created: String,
    pub updated: String,
    pub entry_ids: Vec<String>,
    pub inlined: usize,
    pub task_bytes: u64,
    pub scope: TaskScope,
    #[serde(default)]
    pub knowledge_id: Option<String>,
    #[serde(default)]
    pub language: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskDetail {
    #[serde(flatten)]
    pub meta: TaskMeta,
    pub task_md: String,
    pub result_md: Option<String>,
    pub entries: Vec<EntrySummary>,
    pub dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub builtin: bool,
}

/// Copy built-in templates into the user directory if they are missing.
pub fn install_templates(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for (id, text) in BUILTIN_TEMPLATES {
        let p = dir.join(format!("{id}.md"));
        if !p.exists() {
            std::fs::write(&p, text)?;
        }
    }
    Ok(())
}

fn comment_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("<!-- {key}:");
    text.lines().take(6).find_map(|l| {
        let l = l.trim();
        l.strip_prefix(&needle)
            .map(|rest| rest.trim_end_matches("-->").trim().to_string())
    })
}

pub fn list_templates(dir: &Path) -> Vec<TemplateInfo> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    files.sort();
    for p in files {
        let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().to_string()) else { continue };
        if stem.starts_with('_') || p.extension().map(|e| e != "md").unwrap_or(true) {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap_or_default();
        out.push(TemplateInfo {
            name: comment_field(&text, "name").unwrap_or_else(|| stem.clone()),
            description: comment_field(&text, "description").unwrap_or_default(),
            path: p.to_string_lossy().to_string(),
            builtin: BUILTIN_TEMPLATES.iter().any(|(id, _)| *id == stem),
            id: stem,
        });
    }
    out
}

fn language_name(code: &str) -> &'static str {
    match code {
        c if c.starts_with("zh") => "中文 (Chinese)",
        c if c.starts_with("ja") => "日本語 (Japanese)",
        c if c.starts_with("ko") => "한국어 (Korean)",
        c if c.starts_with("de") => "Deutsch (German)",
        c if c.starts_with("fr") => "Français (French)",
        c if c.starts_with("es") => "Español (Spanish)",
        _ => "English",
    }
}

fn task_dir(hub: &Hub, id: &str) -> PathBuf {
    hub.paths.tasks.join(id)
}

fn read_meta(dir: &Path) -> Option<TaskMeta> {
    let text = std::fs::read_to_string(dir.join("task.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_meta(dir: &Path, meta: &TaskMeta) -> Result<()> {
    std::fs::write(dir.join("task.json"), serde_json::to_string_pretty(meta)?)?;
    Ok(())
}

/// Resolve the entries selected by a scope (newest first).
pub fn resolve_scope(hub: &Hub, scope: &TaskScope) -> Result<Vec<EntrySummary>> {
    let since = scope.since.as_ref().filter(|s| !s.trim().is_empty()).map(|s| {
        if s.len() == 10 { format!("{s}T00:00:00") } else { s.clone() }
    });
    let filter = EntryFilter {
        agent: if scope.agents.len() == 1 { Some(scope.agents[0].clone()) } else { None },
        project: if scope.projects.len() == 1 { Some(scope.projects[0].clone()) } else { None },
        kind: if scope.kinds.len() == 1 { Kind::parse(&scope.kinds[0]) } else { None },
        since,
        include_archived: scope.include_archived,
        limit: Some(5000),
        ..Default::default()
    };
    let mut entries = match scope.query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => hub.search(q, &filter)?,
        None => hub.list(&filter)?,
    };
    let agents: HashSet<&str> = scope.agents.iter().map(|s| s.as_str()).collect();
    let projects: HashSet<&str> = scope.projects.iter().map(|s| s.as_str()).collect();
    let kinds: HashSet<&str> = scope.kinds.iter().map(|s| s.as_str()).collect();
    entries.retain(|e| {
        (agents.is_empty() || agents.contains(e.agent.as_str()))
            && (projects.is_empty() || projects.contains(e.project.as_str()))
            && (kinds.is_empty() || kinds.contains(e.kind.as_str()))
            && (scope.include_knowledge || e.kind != Kind::Knowledge)
    });
    entries.sort_by(|a, b| b.updated.cmp(&a.updated));
    entries.truncate(scope.limit.unwrap_or(200).clamp(1, 2000));
    Ok(entries)
}

fn scope_text(scope: &TaskScope) -> String {
    let mut parts = Vec::new();
    let show = |name: &str, v: &Vec<String>| if v.is_empty() { format!("- {name}: all") } else { format!("- {name}: {}", v.join(", ")) };
    parts.push(show("agents", &scope.agents));
    parts.push(show("projects", &scope.projects));
    parts.push(show("kinds", &scope.kinds));
    if let Some(q) = scope.query.as_deref().filter(|q| !q.trim().is_empty()) {
        parts.push(format!("- search query: `{q}`"));
    }
    if let Some(s) = scope.since.as_deref().filter(|s| !s.trim().is_empty()) {
        parts.push(format!("- updated since: {s}"));
    }
    parts.join("\n")
}

pub fn create(hub: &Hub, template_id: &str, title: Option<String>, scope: TaskScope) -> Result<TaskMeta> {
    let tpl_path = hub.paths.templates.join(format!("{template_id}.md"));
    let template = std::fs::read_to_string(&tpl_path)
        .with_context(|| format!("template not found: {}", tpl_path.display()))?;
    let footer = std::fs::read_to_string(hub.paths.templates.join("_footer.md"))
        .unwrap_or_else(|_| BUILTIN_TEMPLATES.iter().find(|(id, _)| *id == "_footer").map(|(_, t)| t.to_string()).unwrap_or_default());
    let cfg = hub.config();
    let entries = resolve_scope(hub, &scope)?;
    if entries.is_empty() {
        return Err(anyhow!("no memory entries match this scope"));
    }

    let ts = now();
    let id = format!(
        "{}-{}",
        ts.chars().filter(|c| c.is_ascii_digit()).take(14).collect::<String>(),
        template_id
    );
    let dir = task_dir(hub, &id);
    std::fs::create_dir_all(&dir)?;

    // Entry table
    let mut table = String::from("| id | agent | project | kind | title | updated |\n|---|---|---|---|---|---|\n");
    for e in &entries {
        table.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            e.id, e.agent, e.project, e.kind, e.title.replace('|', "/"), e.updated.chars().take(10).collect::<String>()
        ));
    }

    // Inline bodies within budget
    let budget = (cfg.task_context_budget_kb as usize).saturating_mul(1024);
    let mut context = String::new();
    let mut inlined = 0usize;
    let mut omitted: Vec<&EntrySummary> = Vec::new();
    for e in &entries {
        let body = hub.index.get_body(&e.id)?.unwrap_or_default();
        let block = format!(
            "### [{}] {} — {} / {} ({}, updated {})\n\n{}\n\n",
            e.id, e.title, e.agent, e.project, e.kind, e.updated.chars().take(10).collect::<String>(), body.trim_end()
        );
        if context.len() + block.len() > budget && inlined > 0 {
            omitted.push(e);
            continue;
        }
        context.push_str(&block);
        inlined += 1;
    }
    let omitted_note = if omitted.is_empty() {
        String::new()
    } else {
        let mut s = format!(
            "> ⚠️ {} entries were **not inlined** to stay within the context budget. If you have the MemHub MCP tools, read them with `memory_read(id)`; otherwise work with the inlined ones and mention the gap:\n",
            omitted.len()
        );
        for e in &omitted {
            s.push_str(&format!("> - `{}` {} ({} / {})\n", e.id, e.title, e.agent, e.project));
        }
        s
    };

    let title = title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| {
        list_templates(&hub.paths.templates)
            .into_iter()
            .find(|t| t.id == template_id)
            .map(|t| format!("{} — {}", t.name, ts.chars().take(10).collect::<String>()))
            .unwrap_or_else(|| id.clone())
    });

    let language = language_name(&cfg.language).to_string();
    let render = |s: &str| {
        s.replace("{{task_id}}", &id)
            .replace("{{title}}", &title)
            .replace("{{date}}", &ts.chars().take(10).collect::<String>())
            .replace("{{language}}", &language)
            .replace("{{scope}}", &scope_text(&scope))
            .replace("{{entry_count}}", &entries.len().to_string())
            .replace("{{entry_table}}", &table)
            .replace("{{omitted_note}}", &omitted_note)
            .replace("{{context}}", &context)
            .replace("{{vault_path}}", &hub.vault.root().to_string_lossy())
    };
    let task_md = format!("{}\n{}", render(template.trim_end()), render(&footer));
    std::fs::write(dir.join("TASK.md"), &task_md)?;

    let meta = TaskMeta {
        id: id.clone(),
        template: template_id.to_string(),
        title,
        status: "pending".into(),
        created: ts.clone(),
        updated: ts,
        entry_ids: entries.iter().map(|e| e.id.clone()).collect(),
        inlined,
        task_bytes: task_md.len() as u64,
        scope,
        knowledge_id: None,
        language,
    };
    write_meta(&dir, &meta)?;
    Ok(meta)
}

pub fn list(hub: &Hub) -> Vec<TaskMeta> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&hub.paths.tasks) {
        for e in rd.filter_map(|e| e.ok()) {
            if e.path().is_dir() {
                if let Some(m) = read_meta(&e.path()) {
                    out.push(m);
                }
            }
        }
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

pub fn get(hub: &Hub, id: &str) -> Result<TaskDetail> {
    let dir = task_dir(hub, id);
    let mut meta = read_meta(&dir).ok_or_else(|| anyhow!("task not found: {id}"))?;
    let task_md = std::fs::read_to_string(dir.join("TASK.md")).unwrap_or_default();
    let result_md = std::fs::read_to_string(dir.join("result.md")).ok().filter(|r| !r.trim().is_empty());
    // A result written directly to result.md (e.g. `claude -p … > result.md`) promotes the task.
    if result_md.is_some() && meta.status == "pending" {
        meta.status = "done".into();
        meta.updated = now();
        write_meta(&dir, &meta)?;
    }
    let entries = meta
        .entry_ids
        .iter()
        .filter_map(|i| hub.index.get(i).ok().flatten())
        .collect();
    Ok(TaskDetail { meta, task_md, result_md, entries, dir: dir.to_string_lossy().to_string() })
}

/// Oldest pending task (used by MCP `summary_task_get` without an id).
pub fn next_pending(hub: &Hub) -> Option<TaskMeta> {
    list(hub).into_iter().filter(|t| t.status == "pending").min_by(|a, b| a.created.cmp(&b.created))
}

pub fn submit_result(hub: &Hub, id: &str, result: &str) -> Result<TaskMeta> {
    let dir = task_dir(hub, id);
    let mut meta = read_meta(&dir).ok_or_else(|| anyhow!("task not found: {id}"))?;
    if result.trim().is_empty() {
        return Err(anyhow!("result is empty"));
    }
    std::fs::write(dir.join("result.md"), result)?;
    if meta.status != "accepted" {
        meta.status = "done".into();
    }
    meta.updated = now();
    write_meta(&dir, &meta)?;
    Ok(meta)
}

/// Copy the result into the vault as a knowledge note.
pub fn accept(hub: &Hub, id: &str, agent: Option<String>) -> Result<EntrySummary> {
    let dir = task_dir(hub, id);
    let mut meta = read_meta(&dir).ok_or_else(|| anyhow!("task not found: {id}"))?;
    let result = std::fs::read_to_string(dir.join("result.md")).map_err(|_| anyhow!("task has no result yet"))?;
    let title = result
        .lines()
        .find_map(|l| l.trim().strip_prefix("# ").map(|t| t.trim().to_string()))
        .unwrap_or_else(|| meta.title.clone());
    let entry = hub.save_knowledge(
        agent.as_deref().unwrap_or("memhub"),
        &title,
        &result,
        vec![meta.template.clone()],
        meta.entry_ids.clone(),
        Some(meta.id.clone()),
        Some(meta.template.clone()),
    )?;
    meta.status = "accepted".into();
    meta.knowledge_id = Some(entry.id.clone());
    meta.updated = now();
    write_meta(&dir, &meta)?;
    Ok(entry)
}

pub fn delete(hub: &Hub, id: &str) -> Result<()> {
    let dir = task_dir(hub, id);
    if dir.is_dir() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}
