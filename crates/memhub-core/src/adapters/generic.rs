//! Generic folder adapter: any directory + glob rules, for custom agents.

use super::util::{slugify, walk};
use super::RawItem;
use crate::config::SourceConfig;
use crate::model::Kind;
use std::path::Path;

pub fn collect(root: &Path, cfg: &SourceConfig) -> Vec<RawItem> {
    let name = slugify(&cfg.display_name());
    let include: Vec<String> = if cfg.include.is_empty() {
        vec!["**/*.md".into()]
    } else {
        cfg.include.clone()
    };
    let kind = cfg.kind.unwrap_or(Kind::Memory);
    walk(root, &include, &cfg.exclude, 12)
        .into_iter()
        .map(|rel| RawItem {
            agent: name.clone(),
            source: "generic".into(),
            origin: root.join(&rel),
            project: name.clone(),
            project_path: Some(root.to_path_buf()),
            kind,
            vault_rel: Path::new(&name).join(&rel),
        })
        .collect()
}
