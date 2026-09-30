//! The vault: a plain Markdown directory that is the single source of truth.

use crate::frontmatter;
use crate::model::Frontmatter;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const AGENTS_DIR: &str = "agents";
pub const INBOX_DIR: &str = "inbox";
pub const KNOWLEDGE_DIR: &str = "knowledge";
pub const RULES_DIR: &str = "rules";

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    pub fn new(root: PathBuf) -> Vault {
        Vault { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure_layout(&self) -> Result<()> {
        for d in [AGENTS_DIR, INBOX_DIR, KNOWLEDGE_DIR, RULES_DIR] {
            std::fs::create_dir_all(self.root.join(d))?;
        }
        let readme = self.root.join("README.md");
        if !readme.exists() {
            std::fs::write(&readme, README)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700));
        }
        Ok(())
    }

    pub fn abs(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    /// Relative vault path with `/` separators.
    pub fn rel_str(&self, abs: &Path) -> Option<String> {
        abs.strip_prefix(&self.root)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
    }

    /// Entries under `inbox/`, `knowledge/` and `rules/` are native (editable); `agents/` are mirrors.
    pub fn is_native(rel: &str) -> bool {
        rel.starts_with(INBOX_DIR) || rel.starts_with(KNOWLEDGE_DIR) || rel.starts_with(RULES_DIR)
    }

    pub fn read(&self, rel: &str) -> Result<(Frontmatter, String)> {
        let path = self.abs(rel);
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        frontmatter::parse(&text).with_context(|| format!("parse {}", path.display()))
    }

    pub fn read_opt(&self, rel: &str) -> Option<(Frontmatter, String)> {
        self.read(rel).ok()
    }

    pub fn write(&self, rel: &str, fm: &Frontmatter, body: &str) -> Result<()> {
        let path = self.abs(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = frontmatter::render(fm, body);
        let tmp = path.with_extension("md.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn remove(&self, rel: &str) -> Result<()> {
        let path = self.abs(rel);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// All entry files (relative paths), excluding README/INDEX helper files.
    pub fn walk_entries(&self) -> Vec<String> {
        let mut out = Vec::new();
        for d in [AGENTS_DIR, INBOX_DIR, KNOWLEDGE_DIR, RULES_DIR] {
            let base = self.root.join(d);
            if !base.is_dir() {
                continue;
            }
            for e in WalkDir::new(&base).follow_links(false).into_iter().filter_map(|e| e.ok()) {
                if !e.file_type().is_file() {
                    continue;
                }
                let name = e.file_name().to_string_lossy();
                if !name.ends_with(".md") || (d == KNOWLEDGE_DIR && name == "INDEX.md") {
                    continue;
                }
                if let Some(rel) = self.rel_str(e.path()) {
                    out.push(rel);
                }
            }
        }
        out.sort();
        out
    }

    pub fn total_bytes(&self) -> u64 {
        WalkDir::new(&self.root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }
}

const README: &str = r#"# MemHub Vault

This directory is the unified memory vault maintained by MemHub.
Every file is plain Markdown with a small YAML frontmatter block.

- `agents/<agent>/<project>/…` — read-only mirrors of memory files owned by each agent
  (Claude Code, Codex, Gemini CLI, OpenClaw, …). Edit the originals, MemHub re-syncs them.
- `inbox/<agent>/…` — notes written through the MemHub MCP server / HTTP API. Editable.
- `knowledge/…` — knowledge distilled from memories by summary tasks. Editable.
  `knowledge/INDEX.md` is regenerated automatically.
- `rules/<scope>/…` — short instructions reviewed by you (status: draft / approved / retired).
  Only approved rules are served to agents (`memhub context`, MCP `rules_get`).

Frontmatter keys: id, agent, source, origin, project, project_path, kind, title,
created, updated, origin_hash, tags, archived, redacted, sources, task, template.

You can point any agent at this directory directly, or connect it through MCP:
`memhub mcp`.
"#;
