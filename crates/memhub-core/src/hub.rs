//! `Hub` ties config, vault and index together and exposes the operations
//! shared by the CLI, the HTTP API, the Tauri commands and the MCP server.

use crate::adapters::{self, DetectedSource};
use crate::config::{Config, Paths};
use crate::index::Index;
use crate::model::*;
use crate::vault::{Vault, INBOX_DIR, KNOWLEDGE_DIR};
use crate::{scan, tasks};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, SecondsFormat};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Hub {
    pub paths: Paths,
    cfg: RwLock<Config>,
    pub vault: Vault,
    pub index: Index,
    pub(crate) last_sync: Mutex<Option<SyncReport>>,
    pub(crate) sync_lock: Mutex<()>,
}

pub fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn fmt_time(t: std::time::SystemTime) -> String {
    DateTime::<Local>::from(t).to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn summary_from_fm(fm: &Frontmatter, rel: &str, size: u64) -> EntrySummary {
    EntrySummary {
        id: fm.id.clone(),
        agent: fm.agent.clone(),
        source: fm.source.clone(),
        project: fm.project.clone(),
        project_path: fm.project_path.clone(),
        kind: fm.kind,
        title: fm.title.clone(),
        vault_path: rel.to_string(),
        origin: fm.origin.clone(),
        created: fm.created.clone(),
        updated: fm.updated.clone(),
        tags: fm.tags.clone(),
        archived: fm.archived,
        native: Vault::is_native(rel),
        size,
        snippet: None,
    }
}

/// First `# Heading` in the text (skipping an embedded frontmatter block), else the file name.
pub fn extract_title(body: &str, path: &Path) -> String {
    let mut text = body;
    if let Some((yaml, rest)) = crate::frontmatter::split(body) {
        // The source has its own frontmatter (e.g. Cursor .mdc): prefer its title/description.
        let map = crate::frontmatter::parse_map(yaml);
        for key in ["title", "name", "description"] {
            if let Some(v) = map.get(key).map(|v| crate::frontmatter::parse_scalar(v)) {
                let v = v.trim();
                if !v.is_empty() {
                    return v.chars().take(120).collect();
                }
            }
        }
        text = rest;
    }
    for line in text.lines().take(40) {
        let l = line.trim();
        if let Some(h) = l.strip_prefix("# ") {
            let h = h.trim().trim_end_matches('#').trim();
            if !h.is_empty() {
                return h.chars().take(120).collect();
            }
        }
    }
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".into())
}

pub(crate) fn file_slug(title: &str) -> String {
    let s = adapters::util::slugify(title).to_lowercase();
    s.chars().take(60).collect::<String>().trim_matches('-').to_string()
}

impl Hub {
    pub fn open() -> Result<Hub> {
        Hub::open_at(Paths::default_home())
    }

    pub fn open_at(home: PathBuf) -> Result<Hub> {
        std::fs::create_dir_all(&home).with_context(|| format!("create {}", home.display()))?;
        let cfg = Config::load_or_create(&home.join("config.toml"))?;
        let paths = Paths::from_home(home, &cfg);
        let vault = Vault::new(paths.vault.clone());
        vault.ensure_layout()?;
        std::fs::create_dir_all(&paths.tasks)?;
        std::fs::create_dir_all(&paths.logs)?;
        tasks::install_templates(&paths.templates)?;
        let index = Index::open(&paths.index)?;
        let hub = Hub {
            paths,
            cfg: RwLock::new(cfg),
            vault,
            index,
            last_sync: Mutex::new(None),
            sync_lock: Mutex::new(()),
        };
        if hub.index.count()? == 0 {
            hub.index.rebuild(&hub.vault)?;
        }
        Ok(hub)
    }

    // ----- config ---------------------------------------------------------

    pub fn config(&self) -> Config {
        self.cfg.read().unwrap().clone()
    }

    /// Persist a new config. Returns `true` if a restart is required (vault moved).
    pub fn update_config(&self, cfg: Config) -> Result<bool> {
        let restart = cfg.vault != self.config().vault;
        cfg.save(&self.paths.config)?;
        *self.cfg.write().unwrap() = cfg;
        Ok(restart)
    }

    pub fn detect_sources(&self) -> Vec<DetectedSource> {
        adapters::detect(&self.config())
    }

    // ----- sync -----------------------------------------------------------

    pub fn sync(&self) -> Result<SyncReport> {
        let report = scan::sync(self)?;
        *self.last_sync.lock().unwrap() = Some(report.clone());
        Ok(report)
    }

    pub fn last_sync(&self) -> Option<SyncReport> {
        self.last_sync.lock().unwrap().clone()
    }

    pub fn reindex(&self) -> Result<usize> {
        self.index.rebuild(&self.vault)
    }

    // ----- read -----------------------------------------------------------

    pub fn stats(&self) -> Result<Stats> {
        self.index.stats(self.vault.total_bytes())
    }

    pub fn tree(&self) -> Result<Vec<TreeNode>> {
        self.index.tree()
    }

    pub fn list(&self, f: &EntryFilter) -> Result<Vec<EntrySummary>> {
        self.index.list(f)
    }

    pub fn search(&self, q: &str, f: &EntryFilter) -> Result<Vec<EntrySummary>> {
        self.index.search(q, f)
    }

    pub fn get(&self, id: &str) -> Result<EntryFull> {
        let summary = self.index.get(id)?.ok_or_else(|| anyhow!("entry not found: {id}"))?;
        let (frontmatter, body) = self.vault.read(&summary.vault_path)?;
        Ok(EntryFull { summary, frontmatter, body })
    }

    // ----- write ----------------------------------------------------------

    /// Update body/title/tags of an entry. Mirrored entries can be edited too,
    /// but the next sync of a changed source file will overwrite the edit.
    pub fn update_entry(&self, id: &str, body: Option<String>, title: Option<String>, tags: Option<Vec<String>>) -> Result<EntrySummary> {
        let summary = self.index.get(id)?.ok_or_else(|| anyhow!("entry not found: {id}"))?;
        let (mut fm, old_body) = self.vault.read(&summary.vault_path)?;
        let body = body.unwrap_or(old_body);
        if let Some(t) = title {
            fm.title = t;
        }
        if let Some(t) = tags {
            fm.tags = t;
        }
        fm.updated = now();
        self.vault.write(&summary.vault_path, &fm, &body)?;
        let size = self.vault.abs(&summary.vault_path).metadata().map(|m| m.len()).unwrap_or(0);
        let s = summary_from_fm(&fm, &summary.vault_path, size);
        self.index.upsert(&s, &body, fm.origin_hash.as_deref())?;
        Ok(s)
    }

    /// Delete a native entry (inbox/knowledge). Mirrored entries cannot be deleted here.
    pub fn delete_entry(&self, id: &str) -> Result<()> {
        let summary = self.index.get(id)?.ok_or_else(|| anyhow!("entry not found: {id}"))?;
        if !summary.native {
            return Err(anyhow!("mirrored entries are read-only; delete the source file instead"));
        }
        self.vault.remove(&summary.vault_path)?;
        self.index.remove(id)?;
        if summary.vault_path.starts_with(KNOWLEDGE_DIR) {
            self.write_knowledge_index()?;
        }
        Ok(())
    }

    /// Write a note into `inbox/<agent>/`.
    pub fn write_note(&self, agent: &str, title: &str, content: &str, tags: Vec<String>, project: Option<String>) -> Result<EntrySummary> {
        let agent = adapters::util::slugify(if agent.trim().is_empty() { "unknown" } else { agent });
        let title = if title.trim().is_empty() { extract_title(content, Path::new("note.md")) } else { title.trim().to_string() };
        let ts = now();
        let day = &ts[..10];
        let id = ulid::Ulid::new().to_string();
        let rel = format!("{INBOX_DIR}/{agent}/{day}-{}-{}.md", file_slug(&title), &id[id.len() - 4..]);
        let fm = Frontmatter {
            id: id.clone(),
            agent: agent.clone(),
            source: "mcp".into(),
            origin: None,
            project: project.filter(|p| !p.trim().is_empty()).map(|p| adapters::util::slugify(&p)).unwrap_or_else(|| "_global".into()),
            project_path: None,
            kind: Kind::Note,
            title,
            created: ts.clone(),
            updated: ts,
            origin_hash: None,
            tags,
            archived: false,
            redacted: false,
            sources: vec![],
            task: None,
            template: None,
            extra: Default::default(),
        };
        let body = format!("{}\n", content.trim_end());
        self.vault.write(&rel, &fm, &body)?;
        let size = self.vault.abs(&rel).metadata().map(|m| m.len()).unwrap_or(0);
        let s = summary_from_fm(&fm, &rel, size);
        self.index.upsert(&s, &body, None)?;
        Ok(s)
    }

    /// Write a knowledge note into `knowledge/<template-or-general>/`.
    #[allow(clippy::too_many_arguments)]
    pub fn save_knowledge(
        &self,
        agent: &str,
        title: &str,
        content: &str,
        tags: Vec<String>,
        sources: Vec<String>,
        task: Option<String>,
        template: Option<String>,
    ) -> Result<EntrySummary> {
        let title = if title.trim().is_empty() { extract_title(content, Path::new("knowledge.md")) } else { title.trim().to_string() };
        let ts = now();
        let id = ulid::Ulid::new().to_string();
        let sub = template.clone().map(|t| adapters::util::slugify(&t)).unwrap_or_else(|| "general".into());
        let rel = format!("{KNOWLEDGE_DIR}/{sub}/{}-{}-{}.md", &ts[..10], file_slug(&title), &id[id.len() - 4..]);
        let fm = Frontmatter {
            id: id.clone(),
            agent: adapters::util::slugify(if agent.trim().is_empty() { "memhub" } else { agent }),
            source: "summary-task".into(),
            origin: None,
            project: "_global".into(),
            project_path: None,
            kind: Kind::Knowledge,
            title,
            created: ts.clone(),
            updated: ts,
            origin_hash: None,
            tags,
            archived: false,
            redacted: false,
            sources,
            task,
            template,
            extra: Default::default(),
        };
        // Strip a frontmatter block the agent may have produced itself.
        let body_text = crate::frontmatter::split(content).map(|(_, b)| b).unwrap_or(content);
        let body = format!("{}\n", body_text.trim_end());
        self.vault.write(&rel, &fm, &body)?;
        let size = self.vault.abs(&rel).metadata().map(|m| m.len()).unwrap_or(0);
        let s = summary_from_fm(&fm, &rel, size);
        self.index.upsert(&s, &body, None)?;
        self.write_knowledge_index()?;
        Ok(s)
    }

    /// Regenerate `knowledge/INDEX.md`.
    pub fn write_knowledge_index(&self) -> Result<()> {
        let f = EntryFilter { kind: Some(Kind::Knowledge), limit: Some(5000), ..Default::default() };
        let items = self.index.list(&f)?;
        let mut out = String::from("# Knowledge Index\n\n_Auto-generated by MemHub. Do not edit — it is rewritten after every change._\n\n");
        let mut current = String::new();
        let mut sorted = items;
        sorted.sort_by(|a, b| a.vault_path.cmp(&b.vault_path));
        for e in sorted {
            let group = e.vault_path.split('/').nth(1).unwrap_or("general").to_string();
            if group != current {
                out.push_str(&format!("\n## {group}\n\n"));
                current = group;
            }
            let rel = e.vault_path.trim_start_matches(&format!("{KNOWLEDGE_DIR}/")).to_string();
            let date: String = e.updated.chars().take(10).collect();
            let tags = if e.tags.is_empty() { String::new() } else { format!(" · {}", e.tags.join(", ")) };
            out.push_str(&format!("- [{}]({}) — {}{}\n", e.title, rel, date, tags));
        }
        std::fs::write(self.vault.abs(&format!("{KNOWLEDGE_DIR}/INDEX.md")), out)?;
        Ok(())
    }
}
