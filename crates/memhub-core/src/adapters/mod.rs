//! Source adapters: discover memory files of known agents (and generic
//! folders) and describe how they map into the vault.
//!
//! Adapters are intentionally *data-heavy, code-light*: most of them are a
//! root directory plus a handful of glob → kind rules. Only a few need
//! custom logic (e.g. decoding Claude Code's project directory names).

pub mod claude_code;
pub mod codex;
pub mod gemini;
pub mod generic;
pub mod openclaw;
pub mod projects;
pub mod util;
pub mod windsurf;

use crate::config::{collapse_tilde, expand_tilde, Config, SourceConfig};
use crate::model::Kind;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// One file discovered by an adapter, before it is mirrored into the vault.
#[derive(Debug, Clone)]
pub struct RawItem {
    pub agent: String,
    pub source: String,
    pub origin: PathBuf,
    pub project: String,
    pub project_path: Option<PathBuf>,
    pub kind: Kind,
    /// Destination relative to `<vault>/agents/`.
    pub vault_rel: PathBuf,
}

/// Static description of a built-in adapter.
pub struct BuiltinAdapter {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub default_root: fn() -> Option<PathBuf>,
    pub collect: fn(root: &Path, cfg: &SourceConfig) -> Vec<RawItem>,
}

pub const BUILTIN: &[BuiltinAdapter] = &[
    BuiltinAdapter {
        id: "claude-code",
        label: "Claude Code",
        description: "~/.claude/CLAUDE.md · projects/*/memory (Auto Memory) · agent-memory/*",
        default_root: claude_code::default_root,
        collect: claude_code::collect,
    },
    BuiltinAdapter {
        id: "codex",
        label: "Codex CLI",
        description: "~/.codex/memories (memory_summary.md, MEMORY.md, rollout_summaries, skills) · AGENTS.md",
        default_root: codex::default_root,
        collect: codex::collect,
    },
    BuiltinAdapter {
        id: "gemini",
        label: "Gemini CLI",
        description: "~/.gemini/GEMINI.md",
        default_root: gemini::default_root,
        collect: gemini::collect,
    },
    BuiltinAdapter {
        id: "openclaw",
        label: "OpenClaw",
        description: "~/.openclaw/workspace*: MEMORY.md · memory/*.md · USER.md/SOUL.md/…",
        default_root: openclaw::default_root,
        collect: openclaw::collect,
    },
    BuiltinAdapter {
        id: "windsurf",
        label: "Windsurf",
        description: "~/.codeium/windsurf/memories",
        default_root: windsurf::default_root,
        collect: windsurf::collect,
    },
];

pub fn builtin(id: &str) -> Option<&'static BuiltinAdapter> {
    BUILTIN.iter().find(|a| a.id == id)
}

/// What the UI shows on the "Sources" page.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedSource {
    pub r#type: String,
    pub name: String,
    pub label: String,
    pub description: String,
    pub root: Option<String>,
    pub installed: bool,
    pub enabled: bool,
    pub configured: bool,
    pub builtin: bool,
}

fn resolve_root(cfg: &SourceConfig) -> Option<PathBuf> {
    match &cfg.root {
        Some(r) if !r.trim().is_empty() => Some(expand_tilde(r)),
        _ => builtin(&cfg.r#type).and_then(|a| (a.default_root)()),
    }
}

/// Auto-detection: built-in adapters (installed or not) + configured generic sources.
pub fn detect(config: &Config) -> Vec<DetectedSource> {
    let mut out = Vec::new();
    for a in BUILTIN {
        let cfg = config
            .sources
            .iter()
            .find(|s| s.r#type == a.id && s.name.is_none());
        let sc = cfg.cloned().unwrap_or(SourceConfig {
            r#type: a.id.into(),
            ..Default::default()
        });
        let root = resolve_root(&sc);
        let installed = root.as_deref().map(|p| p.exists()).unwrap_or(false);
        out.push(DetectedSource {
            r#type: a.id.into(),
            name: a.id.into(),
            label: a.label.into(),
            description: a.description.into(),
            root: root.as_deref().map(collapse_tilde),
            installed,
            enabled: cfg.map(|c| c.enabled).unwrap_or(false),
            configured: cfg.is_some(),
            builtin: true,
        });
    }
    for s in config.sources.iter().filter(|s| s.r#type == "generic") {
        let root = resolve_root(s);
        out.push(DetectedSource {
            r#type: "generic".into(),
            name: s.display_name(),
            label: s.display_name(),
            description: format!(
                "{} → {}",
                if s.include.is_empty() {
                    "**/*.md".to_string()
                } else {
                    s.include.join(", ")
                },
                s.kind.map(|k| k.as_str()).unwrap_or("memory")
            ),
            root: root.as_deref().map(collapse_tilde),
            installed: root.as_deref().map(|p| p.exists()).unwrap_or(false),
            enabled: s.enabled,
            configured: true,
            builtin: false,
        });
    }
    out
}

/// Roots that the file watcher should observe.
pub fn watch_roots(config: &Config) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for s in config.sources.iter().filter(|s| s.enabled) {
        if let Some(r) = resolve_root(s) {
            if r.exists() {
                roots.push(r);
            }
        }
    }
    for p in &config.projects {
        let path = expand_tilde(&p.path);
        if path.exists() {
            roots.push(path);
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

/// Run every enabled source and return all discovered items.
pub fn collect_all(config: &Config) -> Vec<RawItem> {
    let mut items = Vec::new();
    for s in config.sources.iter().filter(|s| s.enabled) {
        let Some(root) = resolve_root(s) else { continue };
        if !root.exists() {
            continue;
        }
        if s.r#type == "generic" {
            items.extend(generic::collect(&root, s));
        } else if let Some(a) = builtin(&s.r#type) {
            items.extend((a.collect)(&root, s));
        }
    }
    items.extend(projects::collect(config));
    // A file may only be mirrored once even if two rules match it.
    let mut seen = std::collections::HashSet::new();
    items.retain(|i| seen.insert(i.origin.clone()));
    items
}
