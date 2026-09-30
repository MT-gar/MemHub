//! Minimal, dependency-free YAML frontmatter reader/writer.
//!
//! We only ever emit a flat `key: value` block where every string that needs
//! quoting is written as a JSON string (JSON is valid YAML), and every list is a
//! JSON array. That keeps the files readable by any YAML tool while avoiding a
//! full YAML dependency. The parser is a little more liberal so that we can
//! also read hand-edited files (`- item` block lists, unquoted flow lists...).

use crate::model::{Frontmatter, Kind};
use anyhow::{anyhow, Result};
use std::collections::BTreeMap;

/// Split `text` into `(yaml, body)` if it starts with a `---` block.
pub fn split(text: &str) -> Option<(&str, &str)> {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = t
        .strip_prefix("---\n")
        .or_else(|| t.strip_prefix("---\r\n"))?;
    let mut idx = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            let yaml = &rest[..idx];
            let body = &rest[idx + line.len()..];
            return Some((yaml, body));
        }
        idx += line.len();
    }
    None
}

/// Parse a flat YAML block into raw string values (lists are re-encoded as JSON arrays).
pub fn parse_map(yaml: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut pending_key: Option<String> = None;
    let mut pending_items: Vec<String> = Vec::new();

    let flush = |key: &mut Option<String>, items: &mut Vec<String>, map: &mut BTreeMap<String, String>| {
        if let Some(k) = key.take() {
            if !items.is_empty() {
                map.insert(k, serde_json::to_string(&items).unwrap_or_else(|_| "[]".into()));
                items.clear();
            } else {
                map.entry(k).or_default();
            }
        }
    };

    for raw_line in yaml.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            if pending_key.is_some() {
                pending_items.push(parse_scalar(item.trim()));
                continue;
            }
        }
        flush(&mut pending_key, &mut pending_items, &mut map);
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let value = v.trim();
            if value.is_empty() {
                pending_key = Some(key);
            } else {
                map.insert(key, value.to_string());
            }
        }
    }
    flush(&mut pending_key, &mut pending_items, &mut map);
    map
}

/// Decode a scalar: JSON-quoted strings, single-quoted strings, or bare words.
pub fn parse_scalar(raw: &str) -> String {
    let raw = raw.trim();
    if raw.starts_with('"') {
        if let Ok(s) = serde_json::from_str::<String>(raw) {
            return s;
        }
    }
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        return raw[1..raw.len() - 1].replace("''", "'");
    }
    if raw == "~" || raw == "null" {
        return String::new();
    }
    raw.to_string()
}

/// Decode a list: JSON array, bare flow list `[a, b]`, or a single scalar.
pub fn parse_list(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw == "[]" {
        return Vec::new();
    }
    if raw.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Vec<serde_json::Value>>(raw) {
            return v
                .into_iter()
                .map(|x| match x {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                })
                .collect();
        }
        let inner = raw.trim_start_matches('[').trim_end_matches(']');
        return inner
            .split(',')
            .map(|s| parse_scalar(s.trim()))
            .filter(|s| !s.is_empty())
            .collect();
    }
    vec![parse_scalar(raw)]
}

fn parse_bool(raw: &str) -> bool {
    matches!(raw.trim(), "true" | "yes" | "on" | "1")
}

const KNOWN_KEYS: &[&str] = &[
    "id", "agent", "source", "origin", "project", "project_path", "kind", "title", "created",
    "updated", "origin_hash", "tags", "archived", "redacted", "sources", "task", "template",
];

/// Parse a full entry file into its frontmatter and body.
pub fn parse(text: &str) -> Result<(Frontmatter, String)> {
    let (yaml, body) = split(text).ok_or_else(|| anyhow!("missing frontmatter"))?;
    let map = parse_map(yaml);
    let get = |k: &str| map.get(k).map(|v| parse_scalar(v)).unwrap_or_default();
    let opt = |k: &str| {
        let v = get(k);
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    };
    let kind = Kind::parse(&get("kind")).ok_or_else(|| anyhow!("invalid kind in frontmatter"))?;
    let id = get("id");
    if id.is_empty() {
        return Err(anyhow!("missing id in frontmatter"));
    }
    let extra = map
        .iter()
        .filter(|(k, _)| !KNOWN_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), parse_scalar(v)))
        .collect();
    let fm = Frontmatter {
        id,
        agent: get("agent"),
        source: get("source"),
        origin: opt("origin"),
        project: {
            let p = get("project");
            if p.is_empty() {
                "_global".into()
            } else {
                p
            }
        },
        project_path: opt("project_path"),
        kind,
        title: get("title"),
        created: get("created"),
        updated: get("updated"),
        origin_hash: opt("origin_hash"),
        tags: map.get("tags").map(|v| parse_list(v)).unwrap_or_default(),
        archived: map.get("archived").map(|v| parse_bool(v)).unwrap_or(false),
        redacted: map.get("redacted").map(|v| parse_bool(v)).unwrap_or(false),
        sources: map.get("sources").map(|v| parse_list(v)).unwrap_or_default(),
        task: opt("task"),
        template: opt("template"),
        extra,
    };
    Ok((fm, body.to_string()))
}

fn needs_quote(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let special = |c: char| ":#\"'[]{}\n\r\t&*!|>%@`,".contains(c);
    s.contains(special)
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s.starts_with('-')
        || s.starts_with('?')
        || matches!(s, "true" | "false" | "null" | "~" | "yes" | "no" | "on" | "off")
        || s.parse::<f64>().is_ok()
}

fn scalar(s: &str) -> String {
    if needs_quote(s) {
        serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s))
    } else {
        s.to_string()
    }
}

fn list(items: &[String]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".into())
}

/// Render an entry (frontmatter + body) to text.
pub fn render(fm: &Frontmatter, body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 512);
    out.push_str("---\n");
    let mut push = |k: &str, v: String| {
        out.push_str(k);
        out.push_str(": ");
        out.push_str(&v);
        out.push('\n');
    };
    push("id", scalar(&fm.id));
    push("agent", scalar(&fm.agent));
    push("source", scalar(&fm.source));
    if let Some(o) = &fm.origin {
        push("origin", scalar(o));
    }
    push("project", scalar(&fm.project));
    if let Some(p) = &fm.project_path {
        push("project_path", scalar(p));
    }
    push("kind", fm.kind.as_str().to_string());
    push("title", scalar(&fm.title));
    push("created", scalar(&fm.created));
    push("updated", scalar(&fm.updated));
    if let Some(h) = &fm.origin_hash {
        push("origin_hash", scalar(h));
    }
    push("tags", list(&fm.tags));
    if fm.archived {
        push("archived", "true".into());
    }
    if fm.redacted {
        push("redacted", "true".into());
    }
    if !fm.sources.is_empty() {
        push("sources", list(&fm.sources));
    }
    if let Some(t) = &fm.task {
        push("task", scalar(t));
    }
    if let Some(t) = &fm.template {
        push("template", scalar(t));
    }
    for (k, v) in &fm.extra {
        push(k, scalar(v));
    }
    out.push_str("---\n");
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Frontmatter {
        Frontmatter {
            id: "01J".into(),
            agent: "claude-code".into(),
            source: "auto-memory".into(),
            origin: Some("/Users/me/.claude/projects/x/memory/MEMORY.md".into()),
            project: "my-app".into(),
            project_path: Some("/Users/me/dev/my-app".into()),
            kind: Kind::Memory,
            title: "Notes: build & deploy".into(),
            created: "2026-09-01T10:12:00+08:00".into(),
            updated: "2026-09-02T10:12:00+08:00".into(),
            origin_hash: Some("sha256:abc".into()),
            tags: vec!["a".into(), "b c".into()],
            archived: false,
            redacted: true,
            sources: vec!["01A".into()],
            task: Some("t1".into()),
            template: None,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn roundtrip() {
        let fm = sample();
        let text = render(&fm, "# Hello\n\nbody: with colon\n");
        let (back, body) = parse(&text).unwrap();
        assert_eq!(fm, back);
        assert_eq!(body, "# Hello\n\nbody: with colon\n");
    }

    #[test]
    fn block_lists_and_quotes() {
        let text = "---\nid: x\nagent: a\nsource: s\nproject: p\nkind: note\ntitle: 'it''s'\ncreated: c\nupdated: u\ntags:\n  - one\n  - \"two\"\n---\nbody";
        let (fm, body) = parse(text).unwrap();
        assert_eq!(fm.title, "it's");
        assert_eq!(fm.tags, vec!["one", "two"]);
        assert_eq!(body, "body");
    }
}
