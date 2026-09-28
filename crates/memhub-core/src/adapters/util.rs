use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    "dist",
    "build",
];

pub fn build_globset(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        let g = GlobBuilder::new(p)
            .literal_separator(true)
            .build()
            .or_else(|_| Glob::new(p))
            .ok()?;
        b.add(g);
    }
    b.build().ok()
}

/// Walk `root` and return files (relative paths) matching `include` and not `exclude`.
pub fn walk(root: &Path, include: &[String], exclude: &[String], max_depth: usize) -> Vec<PathBuf> {
    let inc = build_globset(include);
    let exc = build_globset(exclude);
    let mut out = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !(e.file_type().is_dir() && SKIP_DIRS.contains(&name.as_ref()))
        });
    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = match entry.path().strip_prefix(root) {
            Ok(r) => r.to_path_buf(),
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if let Some(inc) = &inc {
            if !inc.is_match(&rel_str) {
                continue;
            }
        }
        if let Some(exc) = &exc {
            if exc.is_match(&rel_str) {
                continue;
            }
        }
        out.push(rel);
    }
    out.sort();
    out
}

/// Make a safe, human-readable slug for directory names in the vault.
pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars() {
        if c.is_alphanumeric() || c == '.' || c == '_' {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let t = out.trim_matches(|c| c == '-' || c == '.').to_string();
    if t.is_empty() {
        "_unnamed".into()
    } else {
        t
    }
}

/// Slug for a project directory: its base name.
pub fn project_slug(path: &Path) -> String {
    path.file_name()
        .map(|n| slugify(&n.to_string_lossy()))
        .filter(|s| s != "_unnamed")
        .unwrap_or_else(|| "_root".into())
}

pub fn is_dated_stem(stem: &str) -> bool {
    // YYYY-MM-DD or YYYY-MM-DD-anything
    let b = stem.as_bytes();
    b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
}

pub fn home() -> Option<PathBuf> {
    crate::config::user_home()
}
