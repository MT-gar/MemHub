//! Claude Code adapter.
//!
//! Layout (verified 2026-09):
//! - `~/.claude/CLAUDE.md`                                  → instruction (global)
//! - `~/.claude/projects/<encoded-cwd>/memory/**/*.md`     → memory (Auto Memory: MEMORY.md + topic files)
//! - `~/.claude/agent-memory/<agent-type>/**/*.md`         → memory (persistent sub-agent memory, user scope)
//!
//! The `<encoded-cwd>` directory name is the project path with `/` replaced by `-`,
//! which is ambiguous when paths contain dashes. We therefore prefer the `cwd`
//! field found in the session transcripts (`*.jsonl`) living next to `memory/`.

use super::util::{project_slug, slugify, walk};
use super::RawItem;
use crate::config::SourceConfig;
use crate::model::Kind;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub fn default_root() -> Option<PathBuf> {
    super::util::home().map(|h| h.join(".claude"))
}

pub fn collect(root: &Path, _cfg: &SourceConfig) -> Vec<RawItem> {
    let mut items = Vec::new();
    let agent = "claude-code";

    let global = root.join("CLAUDE.md");
    if global.is_file() {
        items.push(RawItem {
            agent: agent.into(),
            source: "claude-code/global-instructions".into(),
            origin: global,
            project: "_global".into(),
            project_path: None,
            kind: Kind::Instruction,
            vault_rel: PathBuf::from("claude-code/_global/CLAUDE.md"),
        });
    }

    // Auto memory per project
    let projects_dir = root.join("projects");
    if projects_dir.is_dir() {
        let mut used_slugs: HashMap<String, PathBuf> = HashMap::new();
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&projects_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default();
        dirs.sort();
        for pdir in dirs {
            let mem_dir = pdir.join("memory");
            if !mem_dir.is_dir() {
                continue;
            }
            let (project_path, mut slug) = resolve_project(&pdir);
            // De-duplicate slugs of different projects sharing a base name.
            if let Some(existing) = used_slugs.get(&slug) {
                if existing != &pdir {
                    let suffix = short_hash(&pdir.to_string_lossy());
                    slug = format!("{slug}-{suffix}");
                }
            }
            used_slugs.insert(slug.clone(), pdir.clone());
            for rel in walk(&mem_dir, &["**/*.md".into()], &[], 6) {
                items.push(RawItem {
                    agent: agent.into(),
                    source: "claude-code/auto-memory".into(),
                    origin: mem_dir.join(&rel),
                    project: slug.clone(),
                    project_path: project_path.clone(),
                    kind: Kind::Memory,
                    vault_rel: Path::new("claude-code").join(&slug).join(&rel),
                });
            }
        }
    }

    // Persistent sub-agent memory (user scope)
    let agent_mem = root.join("agent-memory");
    if agent_mem.is_dir() {
        for rel in walk(&agent_mem, &["**/*.md".into()], &[], 6) {
            let sub = rel
                .components()
                .next()
                .map(|c| slugify(&c.as_os_str().to_string_lossy()))
                .unwrap_or_else(|| "_agents".into());
            items.push(RawItem {
                agent: agent.into(),
                source: "claude-code/agent-memory".into(),
                origin: agent_mem.join(&rel),
                project: format!("_agents/{sub}"),
                project_path: None,
                kind: Kind::Memory,
                vault_rel: Path::new("claude-code/_agents").join(&rel),
            });
        }
    }

    items
}

/// Returns (project_path, slug) for a `~/.claude/projects/<encoded>` directory.
fn resolve_project(pdir: &Path) -> (Option<PathBuf>, String) {
    if let Some(cwd) = cwd_from_transcripts(pdir) {
        let slug = project_slug(&cwd);
        return (Some(cwd), slug);
    }
    let name = pdir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // Naive decode: "-Users-me-dev-app" → "/Users/me/dev/app". Only trust it if it exists.
    let decoded = PathBuf::from(name.replace('-', "/"));
    if !name.is_empty() && decoded.is_dir() {
        let slug = project_slug(&decoded);
        return (Some(decoded), slug);
    }
    let trimmed = name.trim_start_matches('-');
    let last = trimmed.rsplit('-').next().unwrap_or(trimmed);
    (None, slugify(if last.is_empty() { trimmed } else { last }))
}

/// Look for a `"cwd":"..."` field in the first lines of any session transcript.
fn cwd_from_transcripts(pdir: &Path) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(pdir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "jsonl").unwrap_or(false))
        .collect();
    files.sort();
    for f in files.into_iter().rev().take(5) {
        let Ok(file) = std::fs::File::open(&f) else { continue };
        let reader = BufReader::new(file);
        for line in reader.lines().take(20).map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
                    if !cwd.is_empty() {
                        return Some(PathBuf::from(cwd));
                    }
                }
            }
        }
    }
    None
}

fn short_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(s.as_bytes());
    format!("{:x}", d)[..6].to_string()
}
