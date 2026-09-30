//! Claude Code adapter.
//!
//! Layout (verified 2026-09):
//! - `~/.claude/CLAUDE.md`                                  → instruction (global)
//! - `~/.claude/projects/<encoded-cwd>/memory/**/*.md`     → memory (Auto Memory: MEMORY.md + topic files)
//! - `~/.claude/agent-memory/<agent-type>/**/*.md`         → memory (persistent sub-agent memory, user scope)
//!
//! The `<encoded-cwd>` directory name is the project path with every character outside
//! `[A-Za-z0-9]` replaced by `-` (separators, `:`, `_`, `.`, and every CJK character),
//! which is lossy. To recover the real path we try, in order:
//! 1. the `cwd` field found in the session transcripts (`*.jsonl`) next to `memory/`;
//! 2. a filesystem-guided decode: walk the real directory tree and match each child's
//!    *encoded* name against the encoded string (exact, handles dashes / CJK / `_`);
//! 3. a readable slug derived from the encoded name itself.

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
    if let Some(decoded) = decode_dir_name(&name) {
        let slug = project_slug(&decoded);
        return (Some(decoded), slug);
    }
    (None, fallback_slug(&name))
}

/// Claude Code's path encoding: every UTF-16 code unit outside `[A-Za-z0-9]` becomes `-`.
pub(crate) fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push('-');
            }
        }
    }
    out
}

/// Recover the real project directory from an encoded name by walking the filesystem.
/// Returns `None` when the project no longer exists (or the name is a UNC/WSL path).
fn decode_dir_name(name: &str) -> Option<PathBuf> {
    let b = name.as_bytes();
    if cfg!(windows) {
        // `C:\dir` → `C--dir`
        if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b'-' && b[2] == b'-' {
            let root = PathBuf::from(format!("{}:\\", b[0] as char));
            return decode_from(&root, &name[3..], 0);
        }
        None
    } else {
        // `/home/me/app` → `-home-me-app`
        let rest = name.strip_prefix('-')?;
        decode_from(Path::new("/"), rest, 0)
    }
}

/// Find a path below `cur` whose components encode to exactly `rest` (joined by `-`).
pub(crate) fn decode_from(cur: &Path, rest: &str, depth: usize) -> Option<PathBuf> {
    if rest.is_empty() {
        return Some(cur.to_path_buf());
    }
    if depth > 32 {
        return None;
    }
    let mut kids: Vec<(String, String)> = std::fs::read_dir(cur)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            let enc = encode_segment(&n);
            (n, enc)
        })
        .collect();
    // Longest encoding first: fewer dead ends; name as tie-break for determinism.
    kids.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    for (name, enc) in kids {
        if rest == enc {
            return Some(cur.join(name));
        }
        if let Some(tail) = rest.strip_prefix(enc.as_str()).and_then(|t| t.strip_prefix('-')) {
            if let Some(p) = decode_from(&cur.join(&name), tail, depth + 1) {
                return Some(p);
            }
        }
    }
    None
}

/// Readable, stable slug when the real path cannot be recovered.
fn fallback_slug(name: &str) -> String {
    let mut s = slugify(name.trim_matches('-'));
    if s.chars().count() > 48 {
        // keep the tail (most specific part), cut at a word boundary
        let tail: String = s.chars().rev().take(48).collect::<Vec<_>>().into_iter().rev().collect();
        s = tail.trim_start_matches(['-', '.']).to_string();
    }
    if s.chars().count() < 4 {
        // e.g. "C" from a path made only of CJK characters: make it distinguishable
        s = format!("{}-{}", s, short_hash(name));
    }
    s
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    #[test]
    fn encoding_matches_claude_scheme() {
        assert_eq!(encode_segment("my_app.v2"), "my-app-v2");
        assert_eq!(encode_segment("项目"), "--");
        assert_eq!(encode_segment("a😀b"), "a--b"); // astral char = 2 UTF-16 units
        assert_eq!(encode_segment("QGIS-Agent"), "QGIS-Agent");
    }

    #[test]
    fn decode_recovers_cjk_underscore_and_dash_paths() {
        let t = TempDir::new("decode");
        let real = t.path().join("项目 x").join("my_app.v2").join("sub-dir");
        std::fs::create_dir_all(&real).unwrap();
        let rest = format!("{}-{}-{}", encode_segment("项目 x"), encode_segment("my_app.v2"), encode_segment("sub-dir"));
        assert_eq!(decode_from(t.path(), &rest, 0), Some(real));
    }

    #[test]
    fn decode_backtracks_over_ambiguous_dashes() {
        let t = TempDir::new("backtrack");
        std::fs::create_dir_all(t.path().join("a-b")).unwrap(); // dead end
        let real = t.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&real).unwrap();
        assert_eq!(decode_from(t.path(), "a-b-c", 0), Some(real));
        assert_eq!(decode_from(t.path(), "a-b-zzz", 0), None);
    }

    #[test]
    fn fallback_slug_is_readable_and_never_a_bare_drive_letter() {
        let s = fallback_slug("C--------QGIS-Agent-old-copy");
        assert_eq!(s, "C-QGIS-Agent-old-copy");
        let bare = fallback_slug("C---------------- ");
        assert!(bare.starts_with("C-") && bare.len() > 3, "{bare}");
        // stable across calls
        assert_eq!(bare, fallback_slug("C---------------- "));
        let long = fallback_slug(&format!("--wsl-localhost-Ubuntu-24-04-home-mt-{}", "projects-".repeat(10)));
        assert!(long.chars().count() <= 48, "{long}");
    }

    #[test]
    fn collect_uses_transcript_cwd_and_reports_each_memory_file() {
        let t = TempDir::new("claude");
        let proj = t.path().join("work").join("shop-api");
        std::fs::create_dir_all(&proj).unwrap();
        let cwd = proj.to_string_lossy().replace('\\', "\\\\");
        t.write("projects/enc-a/s1.jsonl", &format!("{{\"type\":\"user\"}}\n{{\"cwd\":\"{cwd}\",\"type\":\"user\"}}\n"));
        t.write("projects/enc-a/memory/MEMORY.md", "# index\n");
        t.write("projects/enc-a/memory/debugging.md", "# dbg\n");
        t.write("projects/enc-b-none/memory/MEMORY.md", "# other\n"); // no transcripts, path not decodable
        t.write("CLAUDE.md", "# global\n");
        t.write("agent-memory/reviewer/MEMORY.md", "# reviewer\n");

        let items = collect(t.path(), &SourceConfig::default());
        let by_project = |p: &str| items.iter().filter(|i| i.project == p).count();
        assert_eq!(by_project("shop-api"), 2, "{items:#?}");
        assert!(items.iter().any(|i| i.source == "claude-code/global-instructions" && i.kind == Kind::Instruction));
        assert!(items.iter().any(|i| i.source == "claude-code/agent-memory"));
        let other = items.iter().find(|i| i.origin.ends_with("enc-b-none/memory/MEMORY.md") || i.origin.to_string_lossy().contains("enc-b-none")).unwrap();
        assert!(other.project.contains("enc-b-none") && other.project_path.is_none(), "{other:?}");
        // vault paths are unique
        let mut rels: Vec<_> = items.iter().map(|i| i.vault_rel.clone()).collect();
        rels.sort();
        rels.dedup();
        assert_eq!(rels.len(), items.len());
    }

    #[test]
    fn same_project_name_in_two_places_gets_distinct_slugs() {
        let t = TempDir::new("claude-dup");
        for (enc, dir) in [("enc-1", "one"), ("enc-2", "two")] {
            let proj = t.path().join(dir).join("app");
            std::fs::create_dir_all(&proj).unwrap();
            let cwd = proj.to_string_lossy().replace('\\', "\\\\");
            t.write(&format!("projects/{enc}/s.jsonl"), &format!("{{\"cwd\":\"{cwd}\"}}\n"));
            t.write(&format!("projects/{enc}/memory/MEMORY.md"), "# m\n");
        }
        let items = collect(t.path(), &SourceConfig::default());
        let mut slugs: Vec<_> = items.iter().map(|i| i.project.clone()).collect();
        slugs.sort();
        slugs.dedup();
        assert_eq!(slugs.len(), 2, "{slugs:?}");
        assert!(slugs.iter().all(|s| s.starts_with("app")));
    }
}
