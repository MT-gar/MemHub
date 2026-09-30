//! Minimal MCP server over stdio (JSON-RPC 2.0, newline-delimited).
//!
//! Implements `initialize`, `ping`, `tools/list`, `tools/call`. Logging must go
//! to stderr — stdout is the protocol channel.

use crate::hub::{Hub, VERSION};
use crate::model::EntryFilter;
use crate::tasks;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::Arc;

const SUPPORTED_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub struct McpServer {
    hub: Arc<Hub>,
    agent: String,
    agent_forced: bool,
}

impl McpServer {
    pub fn new(hub: Arc<Hub>, agent: Option<String>) -> McpServer {
        McpServer { agent_forced: agent.is_some(), agent: agent.unwrap_or_else(|| "mcp-client".into()), hub }
    }

    /// Block on stdin until EOF.
    pub fn run_stdio(mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let msg: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    let err = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}});
                    writeln!(stdout.lock(), "{err}")?;
                    continue;
                }
            };
            if let Some(resp) = self.handle(msg) {
                let mut out = stdout.lock();
                writeln!(out, "{resp}")?;
                out.flush()?;
            }
        }
        Ok(())
    }

    /// Handle one message; notifications return `None`.
    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        if method.is_empty() {
            return None; // a response to a server→client request; we never send those
        }
        let is_notification = id.is_none() || method.starts_with("notifications/");
        let result = match method.as_str() {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => Ok(self.call_tool(&params)),
            "resources/list" => Ok(json!({ "resources": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            m if m.starts_with("notifications/") => return None,
            other => Err((-32601, format!("method not found: {other}"))),
        };
        if is_notification {
            return None;
        }
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
        })
    }

    fn initialize(&mut self, params: &Value) -> Value {
        if !self.agent_forced {
            if let Some(name) = params.pointer("/clientInfo/name").and_then(|n| n.as_str()) {
                self.agent = crate::adapters::util::slugify(name).to_lowercase();
            }
        }
        let requested = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("");
        let version = if SUPPORTED_VERSIONS.contains(&requested) { requested } else { SUPPORTED_VERSIONS[1] };
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "memhub", "version": VERSION },
            "instructions": "MemHub is the unified local memory vault of all the user's AI agents. Use memory_search / memory_read to recall context from other tools and past sessions, memory_write to share something worth remembering, rules_get at the start of a session to load the user's approved preferences (rule_propose to suggest a new one; only the user can approve), and summary_task_get / summary_task_submit when the user asks you to run a MemHub summary task."
        })
    }

    fn call_tool(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        match self.exec(name, &args) {
            Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
            Err(e) => json!({ "content": [{ "type": "text", "text": format!("error: {e}") }], "isError": true }),
        }
    }

    fn exec(&self, name: &str, a: &Value) -> Result<String> {
        let s = |k: &str| a.get(k).and_then(|v| v.as_str()).map(|v| v.to_string()).filter(|v| !v.trim().is_empty());
        let list = |k: &str| -> Vec<String> {
            a.get(k)
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default()
        };
        let limit = a.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);
        let hub = &self.hub;
        match name {
            "memory_search" => {
                let q = s("query").ok_or_else(|| anyhow!("query is required"))?;
                let f = EntryFilter {
                    agent: s("agent"),
                    project: s("project"),
                    kind: s("kind").and_then(|k| crate::model::Kind::parse(&k)),
                    limit: Some(limit.unwrap_or(10)),
                    ..Default::default()
                };
                let hits = hub.search(&q, &f)?;
                if hits.is_empty() {
                    return Ok("No matching memories.".into());
                }
                let mut out = format!("{} result(s):\n\n", hits.len());
                for h in hits {
                    out.push_str(&format!(
                        "- id: {}\n  title: {}\n  agent: {} · project: {} · kind: {} · updated: {}\n",
                        h.id, h.title, h.agent, h.project, h.kind, h.updated
                    ));
                    if let Some(sn) = h.snippet {
                        out.push_str(&format!("  snippet: {}\n", sn.replace('\n', " ")));
                    }
                }
                Ok(out)
            }
            "memory_read" => {
                let id = s("id").ok_or_else(|| anyhow!("id is required"))?;
                let e = hub.get(&id)?;
                Ok(format!(
                    "# {}\n(id: {} · agent: {} · project: {} · kind: {} · updated: {} · path: {})\n\n{}",
                    e.summary.title, e.summary.id, e.summary.agent, e.summary.project, e.summary.kind, e.summary.updated, e.summary.vault_path, e.body
                ))
            }
            "memory_list" => {
                let f = EntryFilter {
                    agent: s("agent"),
                    project: s("project"),
                    kind: s("kind").and_then(|k| crate::model::Kind::parse(&k)),
                    since: s("since"),
                    limit: Some(limit.unwrap_or(20)),
                    ..Default::default()
                };
                let items = hub.list(&f)?;
                let mut out = format!("{} entr{}:\n", items.len(), if items.len() == 1 { "y" } else { "ies" });
                for h in items {
                    out.push_str(&format!("- {} | {} | {} / {} | {} | {}\n", h.id, h.title, h.agent, h.project, h.kind, h.updated));
                }
                Ok(out)
            }
            "memory_write" => {
                let content = s("content").ok_or_else(|| anyhow!("content is required"))?;
                let title = s("title").unwrap_or_default();
                let agent = s("agent").unwrap_or_else(|| self.agent.clone());
                let e = hub.write_note(&agent, &title, &content, list("tags"), s("project"))?;
                Ok(format!("Saved memory {} at {}", e.id, e.vault_path))
            }
            "knowledge_save" => {
                let content = s("content").ok_or_else(|| anyhow!("content is required"))?;
                let title = s("title").unwrap_or_default();
                let e = hub.save_knowledge(&self.agent, &title, &content, list("tags"), list("sources"), None, None)?;
                Ok(format!("Saved knowledge note {} at {}", e.id, e.vault_path))
            }
            "summary_task_get" => {
                let meta = match s("task_id") {
                    Some(id) => tasks::get(hub, &id)?.meta,
                    None => tasks::next_pending(hub).ok_or_else(|| anyhow!("no pending summary task; create one in the MemHub app first"))?,
                };
                let detail = tasks::get(hub, &meta.id)?;
                Ok(format!(
                    "TASK ID: {}\nSTATUS: {}\nWhen finished, call summary_task_submit with task_id=\"{}\" and the full Markdown as result.\n\n{}",
                    meta.id, meta.status, meta.id, detail.task_md
                ))
            }
            "summary_task_submit" => {
                let id = s("task_id").ok_or_else(|| anyhow!("task_id is required"))?;
                let result = s("result").ok_or_else(|| anyhow!("result is required"))?;
                let meta = tasks::submit_result(hub, &id, &result)?;
                Ok(format!("Result stored for task {} (status: {}). The user can review and accept it in MemHub.", meta.id, meta.status))
            }
            "rules_get" => {
                let mut opts = crate::rules::CompileOpts::default();
                if let Some(n) = a.get("max_lines").and_then(|v| v.as_u64()) {
                    opts.max_lines = (n as usize).clamp(1, 500);
                }
                let c = crate::rules::compile(hub, s("project").as_deref(), &opts)?;
                if s("format").as_deref() == Some("json") {
                    return Ok(serde_json::to_string_pretty(&c)?);
                }
                if c.markdown.is_empty() {
                    return Ok("No approved rules yet. (The user approves rules in the MemHub app; rule_propose can suggest one.)".into());
                }
                Ok(c.markdown)
            }
            "rule_propose" => {
                let text = s("text").ok_or_else(|| anyhow!("text is required"))?;
                let agent = s("agent").unwrap_or_else(|| self.agent.clone());
                let scope = s("project").map(|p| format!("project:{}", crate::rules::resolve_project(hub, &p)));
                let o = crate::rules::add(
                    hub,
                    crate::rules::NewRule { text, detail: s("rationale"), scope, sources: list("sources"), status: Some(crate::rules::Status::Draft), agent, ..Default::default() },
                )?;
                let r = o.rule;
                Ok(if o.created {
                    format!("Proposed rule {} as a DRAFT. It is not active: the user must approve it in the MemHub app before any agent receives it.", r.id)
                } else {
                    format!("An identical rule already exists ({} · status: {}). Nothing was changed.", r.id, r.status.as_str())
                })
            }
            other => Err(anyhow!("unknown tool: {other}")),
        }
    }
}

fn tool_definitions() -> Value {
    let str_prop = |d: &str| json!({ "type": "string", "description": d });
    json!([
        {
            "name": "memory_search",
            "description": "Full-text search across the unified memory vault (memories of all agents, shared notes and distilled knowledge). Returns ids to use with memory_read.",
            "inputSchema": { "type": "object", "properties": {
                "query": str_prop("Search terms (all terms must appear)"),
                "agent": str_prop("Optional agent filter, e.g. claude-code, codex, openclaw"),
                "project": str_prop("Optional project slug filter"),
                "kind": str_prop("Optional kind: instruction | memory | daily-log | session-summary | profile | note | knowledge"),
                "limit": { "type": "integer", "description": "Max results (default 10)" }
            }, "required": ["query"] }
        },
        {
            "name": "memory_read",
            "description": "Read the full content of one memory entry by id.",
            "inputSchema": { "type": "object", "properties": { "id": str_prop("Entry id from memory_search / memory_list") }, "required": ["id"] }
        },
        {
            "name": "memory_list",
            "description": "List recent memory entries, optionally filtered by agent / project / kind / since (RFC3339 or YYYY-MM-DD).",
            "inputSchema": { "type": "object", "properties": {
                "agent": str_prop("Agent filter"), "project": str_prop("Project filter"), "kind": str_prop("Kind filter"),
                "since": str_prop("Only entries updated after this time"),
                "limit": { "type": "integer", "description": "Max results (default 20)" }
            } }
        },
        {
            "name": "memory_write",
            "description": "Save a note into the shared vault (inbox) so other agents can find it. Use for durable facts, decisions, preferences or lessons worth remembering across tools.",
            "inputSchema": { "type": "object", "properties": {
                "title": str_prop("Short title"),
                "content": str_prop("Markdown content"),
                "tags": { "type": "array", "items": { "type": "string" } },
                "project": str_prop("Optional project slug"),
                "agent": str_prop("Optional: override the writing agent name")
            }, "required": ["content"] }
        },
        {
            "name": "knowledge_save",
            "description": "Save a distilled knowledge note (lessons, conventions, project brief) into vault/knowledge.",
            "inputSchema": { "type": "object", "properties": {
                "title": str_prop("Title"), "content": str_prop("Markdown content"),
                "tags": { "type": "array", "items": { "type": "string" } },
                "sources": { "type": "array", "items": { "type": "string" }, "description": "Ids of the memory entries this was distilled from" }
            }, "required": ["content"] }
        },
        {
            "name": "rules_get",
            "description": "Load the user's APPROVED rules (personal preferences and per-project conventions) as compact Markdown. Call it once at the start of a session and follow what it says, in addition to the project's own instructions. Drafts and retired rules are never returned.",
            "inputSchema": { "type": "object", "properties": {
                "project": str_prop("Project slug, or the working directory path; adds that project's rules to the global ones"),
                "format": str_prop("markdown (default) or json"),
                "max_lines": { "type": "integer", "description": "Max rules to return (default 40)" }
            } }
        },
        {
            "name": "rule_propose",
            "description": "Suggest ONE new rule (a short imperative sentence, e.g. 'Use pnpm, not npm') when the user states a lasting preference. It is saved as a draft and has no effect until the user approves it in the MemHub app. Do not propose secrets, commands, or one-off task details.",
            "inputSchema": { "type": "object", "properties": {
                "text": str_prop("The rule: one short sentence, max 300 characters"),
                "rationale": str_prop("Optional: why (shown to the user during review, never served)"),
                "project": str_prop("Optional project slug; omit for a global personal preference"),
                "sources": { "type": "array", "items": { "type": "string" }, "description": "Optional memory entry ids that support the rule" }
            }, "required": ["text"] }
        },
        {
            "name": "summary_task_get",
            "description": "Fetch a MemHub summary task (the full prompt with the memory entries to summarise). Without task_id returns the oldest pending task. Follow the instructions in the task and finish with summary_task_submit.",
            "inputSchema": { "type": "object", "properties": { "task_id": str_prop("Task id (optional)") } }
        },
        {
            "name": "summary_task_submit",
            "description": "Submit the Markdown result of a summary task. The user reviews and accepts it in the MemHub app.",
            "inputSchema": { "type": "object", "properties": {
                "task_id": str_prop("Task id"), "result": str_prop("Complete Markdown document")
            }, "required": ["task_id", "result"] }
        }
    ])
}
