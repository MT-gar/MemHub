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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn include_exclude_and_kind_are_honoured() {
        let t = TempDir::new("generic");
        t.write("notes/a.md", "a");
        t.write("notes/b.txt", "b");
        t.write("notes/drafts/c.md", "c");
        t.write("logs/d.md", "d");
        let cfg = SourceConfig {
            r#type: "generic".into(),
            name: Some("My Bot".into()),
            include: vec!["notes/**/*.md".into(), "**/*.txt".into()],
            exclude: vec!["**/drafts/**".into()],
            kind: Some(Kind::DailyLog),
            enabled: true,
            ..Default::default()
        };
        let items = collect(t.path(), &cfg);
        let mut names: Vec<String> = items.iter().map(|i| i.vault_rel.to_string_lossy().replace('\\', "/")).collect();
        names.sort();
        assert_eq!(names, vec!["My-Bot/notes/a.md", "My-Bot/notes/b.txt"]);
        assert!(items.iter().all(|i| i.kind == Kind::DailyLog && i.agent == "My-Bot"));
    }

    #[test]
    fn default_include_is_all_markdown() {
        let t = TempDir::new("generic-default");
        t.write("x.md", "x");
        t.write("y.json", "{}");
        let cfg = SourceConfig { r#type: "generic".into(), name: Some("bot".into()), enabled: true, ..Default::default() };
        assert_eq!(collect(t.path(), &cfg).len(), 1);
    }
}
