//! Core data model shared by the vault, the index, the HTTP/Tauri API and MCP.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Type of a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Human-written instructions for an agent (CLAUDE.md, AGENTS.md, rules...)
    Instruction,
    /// Long-term memory written by the agent itself (MEMORY.md, topic files...)
    Memory,
    /// Append-only dated logs (OpenClaw memory/YYYY-MM-DD.md)
    DailyLog,
    /// Summaries of past sessions (Codex rollout summaries...)
    SessionSummary,
    /// User / persona profiles (OpenClaw USER.md, SOUL.md...)
    Profile,
    /// Free-form notes written through MCP / HTTP into the inbox
    Note,
    /// Knowledge distilled by summary tasks
    Knowledge,
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Instruction,
        Kind::Memory,
        Kind::DailyLog,
        Kind::SessionSummary,
        Kind::Profile,
        Kind::Note,
        Kind::Knowledge,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Instruction => "instruction",
            Kind::Memory => "memory",
            Kind::DailyLog => "daily-log",
            Kind::SessionSummary => "session-summary",
            Kind::Profile => "profile",
            Kind::Note => "note",
            Kind::Knowledge => "knowledge",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|k| k.as_str() == s.trim())
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// YAML frontmatter stored at the top of every vault entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Frontmatter {
    pub id: String,
    pub agent: String,
    pub source: String,
    #[serde(default)]
    pub origin: Option<String>,
    pub project: String,
    #[serde(default)]
    pub project_path: Option<String>,
    pub kind: Kind,
    pub title: String,
    pub created: String,
    pub updated: String,
    #[serde(default)]
    pub origin_hash: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub redacted: bool,
    /// For knowledge notes: ids of the entries this note was distilled from.
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub task: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    /// Unknown keys are preserved verbatim for round-tripping.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

/// Light-weight row used in lists, trees and search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrySummary {
    pub id: String,
    pub agent: String,
    pub source: String,
    pub project: String,
    #[serde(default)]
    pub project_path: Option<String>,
    pub kind: Kind,
    pub title: String,
    /// Path relative to the vault root, always with `/` separators.
    pub vault_path: String,
    #[serde(default)]
    pub origin: Option<String>,
    pub created: String,
    pub updated: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub archived: bool,
    /// `true` for entries that live natively in the vault (inbox/, knowledge/).
    /// Mirrored entries (agents/) are read-only copies of agent files.
    pub native: bool,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryFull {
    #[serde(flatten)]
    pub summary: EntrySummary,
    pub frontmatter: Frontmatter,
    pub body: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntryFilter {
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub kind: Option<Kind>,
    #[serde(default)]
    pub tag: Option<String>,
    /// RFC3339 lower bound on `updated`.
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub total: usize,
    pub archived: usize,
    pub by_agent: Vec<(String, usize)>,
    pub by_kind: Vec<(String, usize)>,
    pub by_project: Vec<(String, usize)>,
    pub last_updated: Option<String>,
    pub vault_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub agent: String,
    pub count: usize,
    pub projects: Vec<TreeProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeProject {
    pub project: String,
    #[serde(default)]
    pub project_path: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncReport {
    pub at: String,
    pub duration_ms: u128,
    pub scanned: usize,
    pub added: usize,
    pub updated: usize,
    pub archived: usize,
    pub unchanged: usize,
    pub errors: Vec<String>,
    pub git_commit: bool,
}
