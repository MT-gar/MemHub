//! OpenClaw adapter.
//!
//! Layout (verified 2026-09): a workspace directory (default `~/.openclaw/workspace`,
//! multi-agent setups use `~/.openclaw/workspace-<agent>`), containing
//! - `MEMORY.md`                → memory (curated long-term memory)
//! - `memory/YYYY-MM-DD.md`     → daily-log
//! - `memory/<topic>.md`        → memory (evergreen topic files)
//! - `USER.md`, `SOUL.md`, `IDENTITY.md`, `TOOLS.md`, `HEARTBEAT.md` → profile
//! - `AGENTS.md`                → instruction

use super::util::{is_dated_stem, slugify, walk};
use super::RawItem;
use crate::config::SourceConfig;
use crate::model::Kind;
use std::path::{Path, PathBuf};

const PROFILE_FILES: &[&str] = &["USER.md", "SOUL.md", "IDENTITY.md", "TOOLS.md", "HEARTBEAT.md"];

/// Default root is the *OpenClaw home*; workspaces are discovered under it.
pub fn default_root() -> Option<PathBuf> {
    super::util::home().map(|h| h.join(".openclaw"))
}

pub fn collect(root: &Path, _cfg: &SourceConfig) -> Vec<RawItem> {
    let mut items = Vec::new();
    // Either the root *is* a workspace, or it contains workspace* directories.
    let mut workspaces: Vec<(String, PathBuf)> = Vec::new();
    if root.join("MEMORY.md").is_file() || root.join("memory").is_dir() || root.join("AGENTS.md").is_file() {
        workspaces.push(("main".into(), root.to_path_buf()));
    } else if let Ok(rd) = std::fs::read_dir(root) {
        let mut dirs: Vec<PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .map(|n| n.to_string_lossy().starts_with("workspace"))
                        .unwrap_or(false)
            })
            .collect();
        dirs.sort();
        for d in dirs {
            let name = d.file_name().unwrap().to_string_lossy().to_string();
            let agent_name = name
                .strip_prefix("workspace-")
                .or_else(|| name.strip_prefix("workspace_"))
                .filter(|s| !s.is_empty())
                .map(slugify)
                .unwrap_or_else(|| "main".into());
            workspaces.push((agent_name, d));
        }
    }

    for (ws_name, ws) in workspaces {
        let base = Path::new("openclaw").join(&ws_name);
        let mem_md = ws.join("MEMORY.md");
        if mem_md.is_file() {
            items.push(item(&ws_name, &ws, "openclaw/long-term", mem_md, Kind::Memory, base.join("MEMORY.md")));
        }
        let agents_md = ws.join("AGENTS.md");
        if agents_md.is_file() {
            items.push(item(&ws_name, &ws, "openclaw/instructions", agents_md, Kind::Instruction, base.join("AGENTS.md")));
        }
        for pf in PROFILE_FILES {
            let p = ws.join(pf);
            if p.is_file() {
                items.push(item(&ws_name, &ws, "openclaw/profile", p, Kind::Profile, base.join("profile").join(pf)));
            }
        }
        let mem_dir = ws.join("memory");
        if mem_dir.is_dir() {
            for rel in walk(&mem_dir, &["**/*.md".into()], &[], 4) {
                let stem = rel.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let (kind, dest) = if is_dated_stem(&stem) {
                    (Kind::DailyLog, base.join("daily").join(&rel))
                } else {
                    (Kind::Memory, base.join("memory").join(&rel))
                };
                items.push(item(&ws_name, &ws, "openclaw/memory-dir", mem_dir.join(&rel), kind, dest));
            }
        }
    }
    items
}

fn item(ws_name: &str, ws: &Path, source: &str, origin: PathBuf, kind: Kind, vault_rel: PathBuf) -> RawItem {
    RawItem {
        agent: "openclaw".into(),
        source: source.into(),
        origin,
        project: ws_name.to_string(),
        project_path: Some(ws.to_path_buf()),
        kind,
        vault_rel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn workspaces_under_home_and_file_kinds() {
        let t = TempDir::new("openclaw");
        t.write("workspace/MEMORY.md", "# long\n");
        t.write("workspace/USER.md", "# user\n");
        t.write("workspace/AGENTS.md", "# agents\n");
        t.write("workspace/memory/2026-09-27.md", "# day\n");
        t.write("workspace/memory/projects.md", "# evergreen\n");
        t.write("workspace-scout/MEMORY.md", "# scout\n");
        t.write("state/ignored.md", "# not a workspace\n");
        let items = collect(t.path(), &SourceConfig::default());
        assert_eq!(items.len(), 6, "{items:#?}");
        assert_eq!(items.iter().filter(|i| i.project == "scout").count(), 1);
        assert!(items.iter().any(|i| i.kind == Kind::DailyLog && i.vault_rel.to_string_lossy().contains("daily")));
        assert!(items.iter().any(|i| i.kind == Kind::Profile));
        assert!(items.iter().all(|i| !i.origin.to_string_lossy().contains("state")));
    }

    #[test]
    fn home_without_workspace_is_empty() {
        let t = TempDir::new("openclaw-state-only");
        t.write("state/x.json", "{}");
        assert!(collect(t.path(), &SourceConfig::default()).is_empty());
    }
}
