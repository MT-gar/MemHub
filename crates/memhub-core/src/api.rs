//! Command dispatcher shared by the HTTP server (`POST /api/<cmd>`) and the
//! Tauri commands (`invoke(cmd, params)`). One JSON in, one JSON out.

use crate::config::{collapse_tilde, Config, ProjectConfig, SourceConfig};
use crate::hub::{Hub, VERSION};
use crate::model::{EntryFilter, Kind};
use crate::rules::{self, CompileOpts, NewRule, Status};
use crate::tasks::{self, TaskScope};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const COMMANDS: &[&str] = &[
    "get_overview", "get_config", "save_config", "list_sources", "add_source", "set_source_enabled",
    "remove_source", "add_project", "remove_project", "sync_now", "reindex", "get_tree",
    "list_entries", "search_entries", "get_entry", "update_entry", "delete_entry", "create_note",
    "list_templates", "create_task", "list_tasks", "get_task", "submit_task_result", "accept_task",
    "delete_task", "get_snippets", "get_paths", "list_rules", "add_rule", "set_rule_status", "edit_rule", "preview_rules",
];

fn parse<T: for<'de> Deserialize<'de>>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|e| anyhow!("invalid params: {e}"))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct IdParams {
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SearchParams {
    q: Option<String>,
    #[serde(flatten)]
    filter: EntryFilter,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct UpdateEntryParams {
    id: String,
    body: Option<String>,
    title: Option<String>,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CreateNoteParams {
    agent: String,
    title: String,
    content: String,
    tags: Vec<String>,
    project: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AddSourceParams {
    r#type: String,
    name: Option<String>,
    root: Option<String>,
    include: Vec<String>,
    exclude: Vec<String>,
    kind: Option<Kind>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SetEnabledParams {
    r#type: String,
    name: Option<String>,
    enabled: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct NameParams {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ProjectParams {
    path: String,
    name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CreateTaskParams {
    template: String,
    title: Option<String>,
    scope: TaskScope,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SubmitParams {
    id: String,
    result: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AcceptParams {
    id: String,
    agent: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SnippetParams {
    /// Path of the `memhub` CLI binary to put into the snippets.
    bin: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListRulesParams {
    scope: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AddRuleParams {
    text: String,
    detail: Option<String>,
    scope: Option<String>,
    sources: Vec<String>,
    /// `approved` (default: a person typing a rule into the GUI is the approval) or `draft`.
    status: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RuleStatusParams {
    id: String,
    status: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EditRuleParams {
    id: String,
    text: Option<String>,
    detail: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PreviewRulesParams {
    project: Option<String>,
    max_lines: Option<usize>,
    with_ids: bool,
}

fn parse_status(s: &str) -> Result<Status> {
    Status::parse(s).ok_or_else(|| anyhow!("invalid status '{s}' (draft | approved | retired)"))
}

#[derive(Deserialize)]
struct SaveConfigParams {
    config: Config,
}

#[derive(Serialize)]
pub struct Snippet {
    pub id: String,
    pub label: String,
    pub file: String,
    pub language: String,
    pub snippet: String,
    pub hint: String,
}

pub fn dispatch(hub: &Hub, cmd: &str, params: Value) -> Result<Value> {
    let params = if params.is_null() { json!({}) } else { params };
    let v = match cmd {
        "get_overview" => json!({
            "version": VERSION,
            "stats": hub.stats()?,
            "sources": hub.detect_sources(),
            "last_sync": hub.last_sync().or_else(|| hub.index.get_meta("last_sync").ok().flatten().and_then(|s| serde_json::from_str(&s).ok())),
            "recent": hub.list(&EntryFilter { limit: Some(12), ..Default::default() })?,
            "pending_rules": rules::list(hub, None, Some(Status::Draft)).map(|v| v.len()).unwrap_or(0),
            "pending_tasks": tasks::list(hub).into_iter().filter(|t| t.status != "accepted").count(),
            "paths": paths_json(hub),
        }),
        "get_paths" => paths_json(hub),
        "get_config" => json!(hub.config()),
        "save_config" => {
            let p: SaveConfigParams = parse(params)?;
            let restart = hub.update_config(p.config)?;
            json!({ "ok": true, "restart_required": restart })
        }
        "list_sources" => json!(hub.detect_sources()),
        "add_source" => {
            let p: AddSourceParams = parse(params)?;
            let mut cfg = hub.config();
            if p.r#type == "generic" {
                let name = p.name.clone().filter(|n| !n.trim().is_empty()).ok_or_else(|| anyhow!("generic source needs a name"))?;
                let root = p.root.clone().filter(|r| !r.trim().is_empty()).ok_or_else(|| anyhow!("generic source needs a root directory"))?;
                cfg.sources.retain(|s| !(s.r#type == "generic" && s.name.as_deref() == Some(name.as_str())));
                cfg.sources.push(SourceConfig { r#type: "generic".into(), name: Some(name), root: Some(root), enabled: true, include: p.include, exclude: p.exclude, kind: p.kind });
            } else {
                crate::adapters::builtin(&p.r#type).ok_or_else(|| anyhow!("unknown source type {}", p.r#type))?;
                if let Some(s) = cfg.sources.iter_mut().find(|s| s.r#type == p.r#type && s.name.is_none()) {
                    s.enabled = true;
                    if p.root.is_some() {
                        s.root = p.root.clone();
                    }
                } else {
                    cfg.sources.push(SourceConfig { r#type: p.r#type.clone(), root: p.root.clone(), enabled: true, ..Default::default() });
                }
            }
            hub.update_config(cfg)?;
            json!(hub.sync()?)
        }
        "set_source_enabled" => {
            let p: SetEnabledParams = parse(params)?;
            let mut cfg = hub.config();
            let mut found = false;
            for s in cfg.sources.iter_mut() {
                let same = if p.r#type == "generic" { s.r#type == "generic" && s.name == p.name } else { s.r#type == p.r#type && s.name.is_none() };
                if same {
                    s.enabled = p.enabled;
                    found = true;
                }
            }
            if !found && p.r#type != "generic" {
                cfg.sources.push(SourceConfig { r#type: p.r#type.clone(), enabled: p.enabled, ..Default::default() });
            }
            hub.update_config(cfg)?;
            json!(hub.sync()?)
        }
        "remove_source" => {
            let p: NameParams = parse(params)?;
            let mut cfg = hub.config();
            cfg.sources.retain(|s| !(s.r#type == "generic" && s.name.as_deref() == Some(p.name.as_str())));
            hub.update_config(cfg)?;
            json!(hub.sync()?)
        }
        "add_project" => {
            let p: ProjectParams = parse(params)?;
            if p.path.trim().is_empty() {
                return Err(anyhow!("path is required"));
            }
            let mut cfg = hub.config();
            cfg.projects.retain(|x| x.path != p.path);
            cfg.projects.push(ProjectConfig { path: p.path, name: p.name.filter(|n| !n.trim().is_empty()) });
            hub.update_config(cfg)?;
            json!(hub.sync()?)
        }
        "remove_project" => {
            let p: ProjectParams = parse(params)?;
            let mut cfg = hub.config();
            cfg.projects.retain(|x| x.path != p.path);
            hub.update_config(cfg)?;
            json!(hub.sync()?)
        }
        "sync_now" => json!(hub.sync()?),
        "reindex" => json!({ "indexed": hub.reindex()? }),
        "get_tree" => json!(hub.tree()?),
        "list_entries" => {
            let f: EntryFilter = parse(params)?;
            json!(hub.list(&f)?)
        }
        "search_entries" => {
            let p: SearchParams = parse(params)?;
            json!(hub.search(p.q.as_deref().unwrap_or(""), &p.filter)?)
        }
        "get_entry" => {
            let p: IdParams = parse(params)?;
            json!(hub.get(&p.id)?)
        }
        "update_entry" => {
            let p: UpdateEntryParams = parse(params)?;
            json!(hub.update_entry(&p.id, p.body, p.title, p.tags)?)
        }
        "delete_entry" => {
            let p: IdParams = parse(params)?;
            hub.delete_entry(&p.id)?;
            json!({ "ok": true })
        }
        "create_note" => {
            let p: CreateNoteParams = parse(params)?;
            json!(hub.write_note(&p.agent, &p.title, &p.content, p.tags, p.project)?)
        }
        "list_templates" => json!(tasks::list_templates(&hub.paths.templates)),
        "create_task" => {
            let p: CreateTaskParams = parse(params)?;
            json!(tasks::create(hub, &p.template, p.title, p.scope)?)
        }
        "list_tasks" => json!(tasks::list(hub)),
        "get_task" => {
            let p: IdParams = parse(params)?;
            json!(tasks::get(hub, &p.id)?)
        }
        "submit_task_result" => {
            let p: SubmitParams = parse(params)?;
            json!(tasks::submit_result(hub, &p.id, &p.result)?)
        }
        "accept_task" => {
            let p: AcceptParams = parse(params)?;
            json!(tasks::accept(hub, &p.id, p.agent)?)
        }
        "delete_task" => {
            let p: IdParams = parse(params)?;
            tasks::delete(hub, &p.id)?;
            json!({ "ok": true })
        }
        "list_rules" => {
            let p: ListRulesParams = parse(params)?;
            let status = match p.status.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => Some(parse_status(s)?),
                None => None,
            };
            json!(rules::list(hub, p.scope.as_deref(), status)?)
        }
        "add_rule" => {
            let p: AddRuleParams = parse(params)?;
            let status = match p.status.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => parse_status(s)?,
                None => Status::Approved,
            };
            let out = rules::add(hub, NewRule { text: p.text, detail: p.detail, scope: p.scope, sources: p.sources, status: Some(status), agent: "user".into(), ..Default::default() })?;
            json!(out)
        }
        "set_rule_status" => {
            let p: RuleStatusParams = parse(params)?;
            json!(rules::set_status(hub, &p.id, parse_status(&p.status)?)?)
        }
        "edit_rule" => {
            let p: EditRuleParams = parse(params)?;
            json!(rules::edit(hub, &p.id, p.text, p.detail)?)
        }
        "preview_rules" => {
            let p: PreviewRulesParams = parse(params)?;
            let mut o = CompileOpts { with_ids: p.with_ids, ..Default::default() };
            if let Some(n) = p.max_lines {
                o.max_lines = n.clamp(1, 500);
            }
            json!(rules::compile(hub, p.project.as_deref(), &o)?)
        }
        "get_snippets" => {
            let p: SnippetParams = parse(params)?;
            json!(snippets(hub, p.bin.as_deref()))
        }
        other => return Err(anyhow!("unknown command: {other}")),
    };
    Ok(v)
}

fn paths_json(hub: &Hub) -> Value {
    json!({
        "home": hub.paths.home.to_string_lossy(),
        "config": hub.paths.config.to_string_lossy(),
        "vault": hub.paths.vault.to_string_lossy(),
        "vault_display": collapse_tilde(&hub.paths.vault),
        "index": hub.paths.index.to_string_lossy(),
        "tasks": hub.paths.tasks.to_string_lossy(),
        "templates": hub.paths.templates.to_string_lossy(),
    })
}

/// MCP / direct-access configuration snippets for the popular agents.
pub fn snippets(hub: &Hub, bin: Option<&str>) -> Vec<Snippet> {
    let bin = bin
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::current_exe().ok().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_else(|| "memhub".into());
    let vault = collapse_tilde(hub.vault.root());
    let json_cfg = format!(
        "{{\n  \"mcpServers\": {{\n    \"memhub\": {{\n      \"command\": \"{bin}\",\n      \"args\": [\"mcp\"]\n    }}\n  }}\n}}"
    );
    vec![
        Snippet {
            id: "claude-code".into(),
            label: "Claude Code".into(),
            file: "terminal".into(),
            language: "bash".into(),
            snippet: format!("claude mcp add --scope user memhub -- \"{bin}\" mcp"),
            hint: "Run once in a terminal. Verify with `claude mcp list`.".into(),
        },
        Snippet {
            id: "codex".into(),
            label: "Codex CLI".into(),
            file: "~/.codex/config.toml".into(),
            language: "toml".into(),
            snippet: format!("[mcp_servers.memhub]\ncommand = \"{bin}\"\nargs = [\"mcp\"]"),
            hint: "Append to ~/.codex/config.toml (or run `codex mcp add memhub -- \"<bin>\" mcp`).".into(),
        },
        Snippet {
            id: "gemini".into(),
            label: "Gemini CLI".into(),
            file: "~/.gemini/settings.json".into(),
            language: "json".into(),
            snippet: json_cfg.clone(),
            hint: "Merge into ~/.gemini/settings.json (or `gemini mcp add memhub \"<bin>\" mcp`).".into(),
        },
        Snippet {
            id: "cursor".into(),
            label: "Cursor".into(),
            file: "~/.cursor/mcp.json".into(),
            language: "json".into(),
            snippet: json_cfg.clone(),
            hint: "Merge into ~/.cursor/mcp.json (global) or <project>/.cursor/mcp.json.".into(),
        },
        Snippet {
            id: "windsurf".into(),
            label: "Windsurf".into(),
            file: "~/.codeium/windsurf/mcp_config.json".into(),
            language: "json".into(),
            snippet: json_cfg.clone(),
            hint: "Merge into the Windsurf MCP config file.".into(),
        },
        Snippet {
            id: "generic-json".into(),
            label: "Any MCP client (Claude Desktop, OpenClaw, …)".into(),
            file: "mcpServers".into(),
            language: "json".into(),
            snippet: json_cfg,
            hint: "Standard stdio MCP server definition; adapt the key name to your client.".into(),
        },
        Snippet {
            id: "direct".into(),
            label: "Direct folder access (no MCP)".into(),
            file: "AGENTS.md / CLAUDE.md / GEMINI.md".into(),
            language: "markdown".into(),
            snippet: format!(
                "## Shared memory vault\n\nA unified memory vault of all my AI agents lives at `{vault}` (plain Markdown, one file per memory, YAML frontmatter).\n- `agents/<agent>/<project>/` mirrors of each agent's own memory files (read-only)\n- `knowledge/` distilled lessons, preferences and project briefs — read `knowledge/INDEX.md` first\n- `inbox/<agent>/` notes shared between agents\nSearch it with grep/rg when you need context from other tools or past sessions."
            ),
            hint: "Paste into any instruction file so agents without MCP still know where the vault is.".into(),
        },
    ]
}
