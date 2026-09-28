//! One sync pass: discover → hash → mirror into the vault → index → archive
//! vanished sources → optional git snapshot.

use crate::adapters;
use crate::gitsnap;
use crate::hub::{extract_title, fmt_time, now, summary_from_fm, Hub};
use crate::model::{Frontmatter, SyncReport};
use crate::redact;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::time::Instant;

pub fn sync(hub: &Hub) -> Result<SyncReport> {
    let _guard = hub.sync_lock.lock().unwrap();
    let start = Instant::now();
    let cfg = hub.config();
    let max_bytes = cfg.max_file_size_kb.saturating_mul(1024);
    let mut report = SyncReport::default();
    let mut live: HashSet<String> = HashSet::new();

    for item in adapters::collect_all(&cfg) {
        report.scanned += 1;
        let rel = format!("agents/{}", item.vault_rel.to_string_lossy().replace('\\', "/"));
        live.insert(rel.clone());

        let meta = match std::fs::metadata(&item.origin) {
            Ok(m) => m,
            Err(e) => {
                report.errors.push(format!("{}: {e}", item.origin.display()));
                continue;
            }
        };
        if meta.len() > max_bytes {
            report.errors.push(format!("{}: skipped ({} KB > max_file_size_kb)", item.origin.display(), meta.len() / 1024));
            continue;
        }
        let raw_bytes = match std::fs::read(&item.origin) {
            Ok(b) => b,
            Err(e) => {
                report.errors.push(format!("{}: {e}", item.origin.display()));
                continue;
            }
        };
        let hash = format!("sha256:{:x}", Sha256::digest(&raw_bytes));
        let raw = String::from_utf8_lossy(&raw_bytes).to_string();

        // Cheap change detection through the index (falls back to the vault file).
        let existing = match hub.index.find_by_vault_path(&rel)? {
            Some(s) => {
                let h = hub.index.origin_hash(&s.id)?;
                Some((s.id, s.created, h, s.archived))
            }
            None => hub
                .vault
                .read_opt(&rel)
                .map(|(fm, _)| (fm.id, fm.created, fm.origin_hash, fm.archived)),
        };
        if let Some((_, _, Some(h), archived)) = &existing {
            if h == &hash && !archived {
                report.unchanged += 1;
                continue;
            }
        }

        let (body, redacted) = if cfg.redact_secrets { redact::redact(&raw) } else { (raw, false) };
        let updated = meta.modified().map(fmt_time).unwrap_or_else(|_| now());
        let created = existing
            .as_ref()
            .map(|(_, c, _, _)| c.clone())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| meta.created().or_else(|_| meta.modified()).map(fmt_time).unwrap_or_else(|_| now()));
        let is_new = existing.is_none();
        let id = existing.map(|(id, _, _, _)| id).unwrap_or_else(|| ulid::Ulid::new().to_string());

        let fm = Frontmatter {
            id,
            agent: item.agent.clone(),
            source: item.source.clone(),
            origin: Some(item.origin.to_string_lossy().replace('\\', "/")),
            project: item.project.clone(),
            project_path: item.project_path.as_ref().map(|p| p.to_string_lossy().replace('\\', "/")),
            kind: item.kind,
            title: extract_title(&body, &item.origin),
            created,
            updated,
            origin_hash: Some(hash.clone()),
            tags: Vec::new(),
            archived: false,
            redacted,
            sources: Vec::new(),
            task: None,
            template: None,
            extra: Default::default(),
        };
        if let Err(e) = hub.vault.write(&rel, &fm, &body) {
            report.errors.push(format!("{rel}: {e}"));
            continue;
        }
        let size = hub.vault.abs(&rel).metadata().map(|m| m.len()).unwrap_or(0);
        let summary = summary_from_fm(&fm, &rel, size);
        hub.index.upsert(&summary, &body, Some(&hash))?;
        if is_new {
            report.added += 1;
        } else {
            report.updated += 1;
        }
    }

    // Archive mirrored entries whose source disappeared.
    for (id, vault_path, _origin) in hub.index.mirrored_live()? {
        if live.contains(&vault_path) {
            continue;
        }
        if let Some((mut fm, body)) = hub.vault.read_opt(&vault_path) {
            fm.archived = true;
            fm.updated = now();
            hub.vault.write(&vault_path, &fm, &body)?;
            let size = hub.vault.abs(&vault_path).metadata().map(|m| m.len()).unwrap_or(0);
            hub.index.upsert(&summary_from_fm(&fm, &vault_path, size), &body, fm.origin_hash.as_deref())?;
        } else {
            hub.index.remove(&id)?;
        }
        report.archived += 1;
    }

    let changed = report.added + report.updated + report.archived;
    if cfg.git_snapshot && changed > 0 {
        match gitsnap::snapshot(hub.vault.root(), &format!("sync: +{} ~{} archived {}", report.added, report.updated, report.archived)) {
            Ok(done) => report.git_commit = done,
            Err(e) => report.errors.push(format!("git snapshot: {e}")),
        }
    }
    report.at = now();
    report.duration_ms = start.elapsed().as_millis();
    hub.index.set_meta("last_sync", &serde_json::to_string(&report)?)?;
    Ok(report)
}
