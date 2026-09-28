//! SQLite (FTS5, trigram) index over the vault. Purely a cache: it can be
//! deleted at any time and is rebuilt from the Markdown files.

use crate::model::{EntryFilter, EntrySummary, Kind, Stats, TreeNode, TreeProject};
use crate::vault::Vault;
use anyhow::{Context, Result};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::Mutex;

pub struct Index {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
CREATE TABLE IF NOT EXISTS entries (
  id TEXT PRIMARY KEY,
  agent TEXT NOT NULL,
  source TEXT NOT NULL,
  project TEXT NOT NULL,
  project_path TEXT,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  vault_path TEXT NOT NULL UNIQUE,
  origin TEXT,
  created TEXT NOT NULL,
  updated TEXT NOT NULL,
  tags TEXT NOT NULL DEFAULT '[]',
  archived INTEGER NOT NULL DEFAULT 0,
  native INTEGER NOT NULL DEFAULT 0,
  size INTEGER NOT NULL DEFAULT 0,
  origin_hash TEXT
);
CREATE INDEX IF NOT EXISTS idx_entries_agent ON entries(agent);
CREATE INDEX IF NOT EXISTS idx_entries_project ON entries(project);
CREATE INDEX IF NOT EXISTS idx_entries_updated ON entries(updated);
CREATE TABLE IF NOT EXISTS bodies (id TEXT PRIMARY KEY, body TEXT NOT NULL);
CREATE VIRTUAL TABLE IF NOT EXISTS fts USING fts5(id UNINDEXED, title, body, tags, tokenize='trigram');
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
"#;

const COLS: &str = "e.id, e.agent, e.source, e.project, e.project_path, e.kind, e.title, e.vault_path, e.origin, e.created, e.updated, e.tags, e.archived, e.native, e.size";

fn row_to_summary(r: &Row<'_>) -> rusqlite::Result<EntrySummary> {
    let kind_s: String = r.get(5)?;
    let tags_s: String = r.get(11)?;
    Ok(EntrySummary {
        id: r.get(0)?,
        agent: r.get(1)?,
        source: r.get(2)?,
        project: r.get(3)?,
        project_path: r.get(4)?,
        kind: Kind::parse(&kind_s).unwrap_or(Kind::Memory),
        title: r.get(6)?,
        vault_path: r.get(7)?,
        origin: r.get(8)?,
        created: r.get(9)?,
        updated: r.get(10)?,
        tags: serde_json::from_str(&tags_s).unwrap_or_default(),
        archived: r.get::<_, i64>(12)? != 0,
        native: r.get::<_, i64>(13)? != 0,
        size: r.get::<_, i64>(14)? as u64,
        snippet: None,
    })
}

/// Build `AND …` clauses + params for an [`EntryFilter`].
fn filter_sql(f: &EntryFilter, params: &mut Vec<String>) -> String {
    let mut sql = String::new();
    if let Some(a) = f.agent.as_deref().filter(|s| !s.is_empty()) {
        params.push(a.to_string());
        sql.push_str(&format!(" AND e.agent = ?{}", params.len()));
    }
    if let Some(p) = f.project.as_deref().filter(|s| !s.is_empty()) {
        params.push(p.to_string());
        sql.push_str(&format!(" AND e.project = ?{}", params.len()));
    }
    if let Some(k) = f.kind {
        params.push(k.as_str().to_string());
        sql.push_str(&format!(" AND e.kind = ?{}", params.len()));
    }
    if let Some(t) = f.tag.as_deref().filter(|s| !s.is_empty()) {
        params.push(format!("%\"{}\"%", t.replace('"', "")));
        sql.push_str(&format!(" AND e.tags LIKE ?{}", params.len()));
    }
    if let Some(s) = f.since.as_deref().filter(|s| !s.is_empty()) {
        params.push(s.to_string());
        sql.push_str(&format!(" AND e.updated >= ?{}", params.len()));
    }
    if !f.include_archived {
        sql.push_str(" AND e.archived = 0");
    }
    sql
}

fn limit_sql(f: &EntryFilter, default_limit: usize) -> String {
    let limit = f.limit.unwrap_or(default_limit).clamp(1, 5000);
    let offset = f.offset.unwrap_or(0);
    format!(" LIMIT {limit} OFFSET {offset}")
}

/// Turn free text into a safe FTS5 query: every whitespace-separated token
/// becomes a quoted phrase (implicit AND).
fn fts_query(q: &str) -> String {
    q.split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Index {
    pub fn open(path: &Path) -> Result<Index> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).with_context(|| format!("open index {}", path.display()))?;
        conn.execute_batch(SCHEMA).context("init index schema")?;
        Ok(Index { conn: Mutex::new(conn) })
    }

    pub fn open_in_memory() -> Result<Index> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Index { conn: Mutex::new(conn) })
    }

    pub fn upsert(&self, e: &EntrySummary, body: &str, origin_hash: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tags = serde_json::to_string(&e.tags)?;
        conn.execute(
            "DELETE FROM entries WHERE vault_path = ?1 AND id != ?2",
            params![e.vault_path, e.id],
        )?;
        conn.execute(
            "INSERT INTO entries (id, agent, source, project, project_path, kind, title, vault_path, origin, created, updated, tags, archived, native, size, origin_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(id) DO UPDATE SET agent=excluded.agent, source=excluded.source, project=excluded.project,
               project_path=excluded.project_path, kind=excluded.kind, title=excluded.title, vault_path=excluded.vault_path,
               origin=excluded.origin, created=excluded.created, updated=excluded.updated, tags=excluded.tags,
               archived=excluded.archived, native=excluded.native, size=excluded.size, origin_hash=excluded.origin_hash",
            params![
                e.id, e.agent, e.source, e.project, e.project_path, e.kind.as_str(), e.title, e.vault_path,
                e.origin, e.created, e.updated, tags, e.archived as i64, e.native as i64, e.size as i64, origin_hash
            ],
        )?;
        conn.execute(
            "INSERT INTO bodies (id, body) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET body = excluded.body",
            params![e.id, body],
        )?;
        conn.execute("DELETE FROM fts WHERE id = ?1", params![e.id])?;
        conn.execute(
            "INSERT INTO fts (id, title, body, tags) VALUES (?1, ?2, ?3, ?4)",
            params![e.id, e.title, body, e.tags.join(" ")],
        )?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM entries WHERE id = ?1", params![id])?;
        conn.execute("DELETE FROM bodies WHERE id = ?1", params![id])?;
        conn.execute("DELETE FROM fts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("DELETE FROM entries; DELETE FROM bodies; DELETE FROM fts;")?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<EntrySummary>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("SELECT {COLS} FROM entries e WHERE e.id = ?1");
        Ok(conn.query_row(&sql, params![id], row_to_summary).optional()?)
    }

    pub fn get_body(&self, id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT body FROM bodies WHERE id = ?1", params![id], |r| r.get(0))
            .optional()?)
    }

    pub fn find_by_vault_path(&self, vault_path: &str) -> Result<Option<EntrySummary>> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("SELECT {COLS} FROM entries e WHERE e.vault_path = ?1");
        Ok(conn.query_row(&sql, params![vault_path], row_to_summary).optional()?)
    }

    pub fn origin_hash(&self, id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT origin_hash FROM entries WHERE id = ?1", params![id], |r| r.get(0))
            .optional()?
            .flatten())
    }

    pub fn list(&self, f: &EntryFilter) -> Result<Vec<EntrySummary>> {
        let conn = self.conn.lock().unwrap();
        let mut p = Vec::new();
        let sql = format!(
            "SELECT {COLS} FROM entries e WHERE 1=1{} ORDER BY e.updated DESC{}",
            filter_sql(f, &mut p),
            limit_sql(f, 200)
        );
        let mut st = conn.prepare(&sql)?;
        let rows = st.query_map(params_from_iter(p.iter()), row_to_summary)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn search(&self, q: &str, f: &EntryFilter) -> Result<Vec<EntrySummary>> {
        let q = q.trim();
        if q.is_empty() {
            return self.list(f);
        }
        let conn = self.conn.lock().unwrap();
        // trigram tokenizer needs >= 3 chars; fall back to LIKE for short queries.
        if q.chars().count() < 3 {
            let mut p = vec![format!("%{}%", q)];
            let sql = format!(
                "SELECT {COLS} FROM entries e JOIN bodies b ON b.id = e.id WHERE (e.title LIKE ?1 OR b.body LIKE ?1){} ORDER BY e.updated DESC{}",
                filter_sql(f, &mut p),
                limit_sql(f, 100)
            );
            let mut st = conn.prepare(&sql)?;
            let rows = st.query_map(params_from_iter(p.iter()), row_to_summary)?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        }
        let mut p = vec![fts_query(q)];
        let sql = format!(
            "SELECT {COLS}, snippet(fts, 2, '[[', ']]', ' … ', 18) AS snip
             FROM fts JOIN entries e ON e.id = fts.id
             WHERE fts MATCH ?1{} ORDER BY bm25(fts, 5.0, 1.0, 2.0){}",
            filter_sql(f, &mut p),
            limit_sql(f, 100)
        );
        let mut st = conn.prepare(&sql)?;
        let rows = st.query_map(params_from_iter(p.iter()), |r| {
            let mut s = row_to_summary(r)?;
            s.snippet = r.get::<_, Option<String>>(15)?;
            Ok(s)
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// (id, vault_path, origin) of live mirrored entries — used to detect deleted sources.
    pub fn mirrored_live(&self) -> Result<Vec<(String, String, Option<String>)>> {
        let conn = self.conn.lock().unwrap();
        let mut st = conn.prepare(
            "SELECT id, vault_path, origin FROM entries WHERE native = 0 AND archived = 0",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn stats(&self, vault_bytes: u64) -> Result<Stats> {
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM entries WHERE archived = 0", [], |r| r.get(0))?;
        let archived: i64 = conn.query_row("SELECT COUNT(*) FROM entries WHERE archived = 1", [], |r| r.get(0))?;
        let group = |col: &str| -> Result<Vec<(String, usize)>> {
            let sql = format!(
                "SELECT {col}, COUNT(*) c FROM entries WHERE archived = 0 GROUP BY {col} ORDER BY c DESC"
            );
            let mut st = conn.prepare(&sql)?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize)))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        };
        let last_updated: Option<String> = conn
            .query_row("SELECT MAX(updated) FROM entries WHERE archived = 0", [], |r| r.get(0))
            .optional()?
            .flatten();
        Ok(Stats {
            total: total as usize,
            archived: archived as usize,
            by_agent: group("agent")?,
            by_kind: group("kind")?,
            by_project: group("project")?,
            last_updated,
            vault_bytes,
        })
    }

    pub fn tree(&self) -> Result<Vec<TreeNode>> {
        let conn = self.conn.lock().unwrap();
        let mut st = conn.prepare(
            "SELECT agent, project, MAX(project_path), COUNT(*) FROM entries WHERE archived = 0 GROUP BY agent, project ORDER BY agent, project",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)? as usize,
            ))
        })?;
        let mut out: Vec<TreeNode> = Vec::new();
        for (agent, project, project_path, count) in rows.filter_map(|r| r.ok()) {
            if out.last().map(|n| n.agent != agent).unwrap_or(true) {
                out.push(TreeNode { agent: agent.clone(), count: 0, projects: Vec::new() });
            }
            let node = out.last_mut().unwrap();
            node.count += count;
            node.projects.push(TreeProject { project, project_path, count });
        }
        Ok(out)
    }

    pub fn count(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Drop everything and re-read the vault from disk.
    pub fn rebuild(&self, vault: &Vault) -> Result<usize> {
        self.clear()?;
        let mut n = 0;
        for rel in vault.walk_entries() {
            let Ok((fm, body)) = vault.read(&rel) else { continue };
            let size = std::fs::metadata(vault.abs(&rel)).map(|m| m.len()).unwrap_or(0);
            let s = crate::hub::summary_from_fm(&fm, &rel, size);
            self.upsert(&s, &body, fm.origin_hash.as_deref())?;
            n += 1;
        }
        Ok(n)
    }
}
