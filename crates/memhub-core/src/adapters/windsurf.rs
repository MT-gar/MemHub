//! Windsurf (Cascade) adapter: `~/.codeium/windsurf/memories/**/*.md`.

use super::util::walk;
use super::RawItem;
use crate::config::SourceConfig;
use crate::model::Kind;
use std::path::{Path, PathBuf};

pub fn default_root() -> Option<PathBuf> {
    super::util::home().map(|h| h.join(".codeium").join("windsurf").join("memories"))
}

pub fn collect(root: &Path, _cfg: &SourceConfig) -> Vec<RawItem> {
    walk(root, &["**/*.md".into()], &[], 5)
        .into_iter()
        .map(|rel| RawItem {
            agent: "windsurf".into(),
            source: "windsurf/memories".into(),
            origin: root.join(&rel),
            project: "_global".into(),
            project_path: None,
            kind: Kind::Memory,
            vault_rel: Path::new("windsurf/_global").join(&rel),
        })
        .collect()
}
