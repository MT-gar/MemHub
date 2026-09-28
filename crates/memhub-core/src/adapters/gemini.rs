//! Gemini CLI adapter: `~/.gemini/GEMINI.md` (global context / `/memory add`).

use super::RawItem;
use crate::config::SourceConfig;
use crate::model::Kind;
use std::path::{Path, PathBuf};

pub fn default_root() -> Option<PathBuf> {
    super::util::home().map(|h| h.join(".gemini"))
}

pub fn collect(root: &Path, _cfg: &SourceConfig) -> Vec<RawItem> {
    let mut items = Vec::new();
    let global = root.join("GEMINI.md");
    if global.is_file() {
        items.push(RawItem {
            agent: "gemini".into(),
            source: "gemini/global-instructions".into(),
            origin: global,
            project: "_global".into(),
            project_path: None,
            kind: Kind::Instruction,
            vault_rel: PathBuf::from("gemini/_global/GEMINI.md"),
        });
    }
    items
}
