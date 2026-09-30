//! OpenAI Codex CLI adapter.
//!
//! Layout (verified 2026-09, requires `[features] memories = true` in ~/.codex/config.toml):
//! - `~/.codex/memories/memory_summary.md`      → memory (injected into the system prompt)
//! - `~/.codex/memories/MEMORY.md`              → memory (long-form registry)
//! - `~/.codex/memories/raw_memories.md`        → memory (pre-consolidation extraction)
//! - `~/.codex/memories/rollout_summaries/*.md` → session-summary
//! - `~/.codex/memories/skills/*/SKILL.md`      → memory
//! - `~/.codex/AGENTS.md`                       → instruction (global)

use super::util::walk;
use super::RawItem;
use crate::config::SourceConfig;
use crate::model::Kind;
use std::path::{Path, PathBuf};

pub fn default_root() -> Option<PathBuf> {
    super::util::home().map(|h| h.join(".codex"))
}

pub fn collect(root: &Path, _cfg: &SourceConfig) -> Vec<RawItem> {
    let mut items = Vec::new();
    let agent = "codex";

    let global = root.join("AGENTS.md");
    if global.is_file() {
        items.push(RawItem {
            agent: agent.into(),
            source: "codex/global-instructions".into(),
            origin: global,
            project: "_global".into(),
            project_path: None,
            kind: Kind::Instruction,
            vault_rel: PathBuf::from("codex/_global/AGENTS.md"),
        });
    }

    let mem = root.join("memories");
    if mem.is_dir() {
        for rel in walk(&mem, &["**/*.md".into()], &[], 6) {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let kind = if rel_str.starts_with("rollout_summaries/") {
                Kind::SessionSummary
            } else {
                Kind::Memory
            };
            items.push(RawItem {
                agent: agent.into(),
                source: "codex/memories".into(),
                origin: mem.join(&rel),
                project: "_global".into(),
                project_path: None,
                kind,
                vault_rel: Path::new("codex/_global").join(&rel),
            });
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn global_instructions_and_memories_are_classified() {
        let t = TempDir::new("codex");
        t.write("AGENTS.md", "# rules\n");
        t.write("AGENT.md", "# legacy singular name is ignored\n");
        t.write("memories/memory_summary.md", "# s\n");
        t.write("memories/MEMORY.md", "# m\n");
        t.write("memories/rollout_summaries/2026-09-01-a.md", "# r\n");
        t.write("memories_1.sqlite", "not a markdown file");
        let items = collect(t.path(), &SourceConfig::default());
        assert_eq!(items.len(), 4, "{items:#?}");
        assert!(items.iter().any(|i| i.kind == Kind::Instruction && i.vault_rel.ends_with("AGENTS.md")));
        assert!(items.iter().any(|i| i.kind == Kind::SessionSummary));
        assert_eq!(items.iter().filter(|i| i.kind == Kind::Memory).count(), 2);
    }

    #[test]
    fn empty_home_yields_nothing() {
        let t = TempDir::new("codex-empty");
        assert!(collect(t.path(), &SourceConfig::default()).is_empty());
    }
}
