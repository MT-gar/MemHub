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
}
