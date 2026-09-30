//! # memhub-core
//!
//! Unified local memory vault for AI agents: discover and mirror the memory
//! files of Claude Code, Codex, Gemini CLI, OpenClaw and friends into one
//! Markdown directory, index them, expose them over MCP, and prepare
//! "bring your own agent" summary tasks.

pub mod adapters;
pub mod api;
pub mod config;
pub mod frontmatter;
pub mod gitsnap;
pub mod hub;
pub mod index;
pub mod mcp;
pub mod model;
pub mod redact;
pub mod scan;
pub mod tasks;
pub mod vault;
pub mod watch;

#[cfg(test)]
pub(crate) mod testutil {
    //! Tiny temp-dir helper (removed on drop) so tests do not need an extra dependency.
    use std::path::{Path, PathBuf};

    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> TempDir {
            let p = std::env::temp_dir().join(format!("memhub-test-{tag}-{}", ulid::Ulid::new()));
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
        /// Write `content` to `rel` below the temp dir (parents are created).
        pub fn write(&self, rel: &str, content: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, content).unwrap();
            p
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

pub use config::{Config, Paths};
pub use hub::{Hub, VERSION};
pub use model::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_home(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("memhub-test-{}-{}", name, ulid::Ulid::new()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn end_to_end_generic_source() {
        let home = temp_home("e2e");
        let src = home.join("agent-a");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("MEMORY.md"), "# Agent A memory\n\n- prefers pnpm over npm\n- api key sk-abcdefghijklmnopqrstuvwxyz123456\n").unwrap();
        fs::write(src.join("sub").join("notes.md"), "debugging tip: run cargo test with RUST_BACKTRACE=1\n").unwrap();

        let hub = Hub::open_at(home.clone()).unwrap();
        let mut cfg = hub.config();
        cfg.git_snapshot = false;
        // hermetic: built-in adapters are enabled by default and would read the real ~/.claude etc.
        cfg.sources.retain(|s| s.r#type == "generic");
        cfg.sources.push(config::SourceConfig {
            r#type: "generic".into(),
            name: Some("agent-a".into()),
            root: Some(src.to_string_lossy().to_string()),
            enabled: true,
            ..Default::default()
        });
        hub.update_config(cfg).unwrap();

        let r = hub.sync().unwrap();
        assert_eq!(r.added, 2, "{r:?}");
        let r2 = hub.sync().unwrap();
        assert_eq!(r2.unchanged, 2);

        // redaction happened
        let hits = hub.search("pnpm", &EntryFilter::default()).unwrap();
        assert_eq!(hits.len(), 1);
        let full = hub.get(&hits[0].id).unwrap();
        assert!(!full.body.contains("sk-abc"));
        assert!(full.frontmatter.redacted);
        assert_eq!(full.summary.title, "Agent A memory");

        // search body of the nested file
        let hits = hub.search("RUST_BACKTRACE", &EntryFilter::default()).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].vault_path, "agents/agent-a/sub/notes.md");

        // note through the MCP path
        let n = hub.write_note("tester", "hello", "shared fact", vec!["t".into()], None).unwrap();
        assert!(n.native);
        assert_eq!(hub.search("shared fact", &EntryFilter::default()).unwrap().len(), 1);

        // summary task round trip
        let t = tasks::create(&hub, "lessons", None, tasks::TaskScope::default()).unwrap();
        assert_eq!(t.entry_ids.len(), 3);
        let d = tasks::get(&hub, &t.id).unwrap();
        assert!(d.task_md.contains("RUST_BACKTRACE"));
        tasks::submit_result(&hub, &t.id, "# Lessons\n\n- use RUST_BACKTRACE [src: x]\n").unwrap();
        let k = tasks::accept(&hub, &t.id, None).unwrap();
        assert_eq!(k.kind, Kind::Knowledge);
        assert!(hub.vault.abs("knowledge/INDEX.md").exists());

        // archive when the source disappears
        fs::remove_file(src.join("sub").join("notes.md")).unwrap();
        let r3 = hub.sync().unwrap();
        assert_eq!(r3.archived, 1);

        // index rebuild from disk yields the same count
        let n_before = hub.index.count().unwrap();
        let n_after = hub.reindex().unwrap();
        assert_eq!(n_before, n_after);

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn mcp_handshake_and_tools() {
        let home = temp_home("mcp");
        let hub = std::sync::Arc::new(Hub::open_at(home.clone()).unwrap());
        let mut srv = mcp::McpServer::new(hub.clone(), None);
        let init = srv
            .handle(serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"Claude Code","version":"1"}}}))
            .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert!(srv.handle(serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let tools = srv.handle(serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 7);
        let w = srv
            .handle(serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"memory_write","arguments":{"title":"from mcp","content":"remember the milk"}}}))
            .unwrap();
        assert_eq!(w["result"]["isError"], false);
        let hits = hub.search("milk", &EntryFilter::default()).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].agent, "claude-code");
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn fresh_config_enables_builtin_sources() {
        let home = temp_home("fresh");
        let hub = Hub::open_at(home.clone()).unwrap();
        let cfg = hub.config();
        assert_eq!(cfg.sources.len(), adapters::BUILTIN.len());
        assert!(cfg.sources.iter().all(|s| s.enabled), "a fresh install must not start with every source disabled");
        // and the flag survives a save / load round trip
        let again = Config::load(&home.join("config.toml")).unwrap();
        assert!(again.sources.iter().all(|s| s.enabled));
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn sync_updates_changed_files_and_skips_oversized() {
        let home = temp_home("sync");
        let src = home.join("agent-b");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.md"), "# A\n\nfirst version\n").unwrap();
        fs::write(src.join("big.md"), "x".repeat(3 * 1024)).unwrap();

        let hub = Hub::open_at(home.clone()).unwrap();
        let mut cfg = hub.config();
        cfg.git_snapshot = false;
        cfg.max_file_size_kb = 1;
        cfg.sources.retain(|s| s.r#type == "generic");
        cfg.sources.push(config::SourceConfig {
            r#type: "generic".into(),
            name: Some("agent-b".into()),
            root: Some(src.to_string_lossy().to_string()),
            enabled: true,
            ..Default::default()
        });
        hub.update_config(cfg).unwrap();

        let r = hub.sync().unwrap();
        assert_eq!((r.added, r.errors.len()), (1, 1), "{r:?}");
        assert!(r.errors[0].contains("skipped"), "{r:?}");

        // change the file -> counted as updated, not added
        fs::write(src.join("a.md"), "# A\n\nsecond version with more words\n").unwrap();
        let r2 = hub.sync().unwrap();
        assert_eq!((r2.added, r2.updated), (0, 1), "{r2:?}");
        assert_eq!(hub.search("second version", &EntryFilter::default()).unwrap().len(), 1);
        assert_eq!(hub.search("first version", &EntryFilter::default()).unwrap().len(), 0);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn cjk_search_and_inbox_notes() {
        let home = temp_home("cjk");
        let hub = Hub::open_at(home.clone()).unwrap();
        hub.write_note("tester", "编码偏好", "回答请使用简体中文，代码注释使用英文。", vec![], None).unwrap();
        // FTS5 trigram tokenizer: 3+ characters match
        assert_eq!(hub.search("简体中文", &EntryFilter::default()).unwrap().len(), 1);
        assert_eq!(hub.search("代码注释", &EntryFilter { kind: Some(Kind::Note), ..Default::default() }).unwrap().len(), 1);
        let _ = fs::remove_dir_all(home);
    }
}
