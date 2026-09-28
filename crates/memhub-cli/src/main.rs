mod serve;

use anyhow::Result;
use clap::{Parser, Subcommand};
use memhub_core::{model::EntryFilter, Hub};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "memhub", version, about = "MemHub — unified local memory vault for AI agents")]
struct Cli {
    /// Override the MemHub home directory (default: $MEMHUB_HOME or ~/.memhub)
    #[arg(long, global = true)]
    home: Option<std::path::PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Discover and mirror agent memories into the vault once
    Scan,
    /// Scan, then keep watching source directories for changes
    Watch {
        /// Periodic full sync interval in seconds (0 = off)
        #[arg(long, default_value_t = 300)]
        interval: u64,
    },
    /// Run the MCP server over stdio (configure this command in your agent)
    Mcp {
        /// Force the agent name used for memories written through this connection
        #[arg(long)]
        agent: Option<String>,
        /// Skip the initial background sync
        #[arg(long)]
        no_sync: bool,
    },
    /// Serve the web UI + HTTP API (browser mode)
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 7337)]
        port: u16,
        /// Do not watch source directories
        #[arg(long)]
        no_watch: bool,
    },
    /// Show detected agents and configured sources
    Detect,
    /// Full-text search
    Search {
        query: String,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// List recent entries
    List {
        #[arg(long)]
        agent: Option<String>,
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// Print an entry
    Show { id: String },
    /// Print resolved paths
    Paths,
    /// Rebuild the search index from the vault files
    Reindex,
}

fn open(home: Option<std::path::PathBuf>) -> Result<Hub> {
    match home {
        Some(h) => Hub::open_at(h),
        None => Hub::open(),
    }
}

fn print_report(r: &memhub_core::SyncReport) {
    println!(
        "scanned {} · added {} · updated {} · archived {} · unchanged {} · {} ms{}",
        r.scanned, r.added, r.updated, r.archived, r.unchanged, r.duration_ms,
        if r.git_commit { " · git commit" } else { "" }
    );
    for e in &r.errors {
        eprintln!("  ! {e}");
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let is_mcp = matches!(cli.cmd, Cmd::Mcp { .. });
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive(
            if is_mcp { "warn".parse()? } else { "info".parse()? },
        ))
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    match cli.cmd {
        Cmd::Scan => {
            let hub = open(cli.home)?;
            print_report(&hub.sync()?);
        }
        Cmd::Watch { interval } => {
            let hub = Arc::new(open(cli.home)?);
            print_report(&hub.sync()?);
            println!("watching {} root(s)… Ctrl-C to stop", memhub_core::adapters::watch_roots(&hub.config()).len());
            let handle = memhub_core::watch::spawn(hub, interval, Some(Arc::new(|r: &memhub_core::SyncReport| {
                if r.added + r.updated + r.archived > 0 {
                    print_report(r);
                }
            })))?;
            wait_ctrl_c();
            handle.stop();
        }
        Cmd::Mcp { agent, no_sync } => {
            let hub = Arc::new(open(cli.home)?);
            if !no_sync {
                let h = hub.clone();
                std::thread::spawn(move || {
                    let _ = h.sync();
                });
            }
            memhub_core::mcp::McpServer::new(hub, agent).run_stdio()?;
        }
        Cmd::Serve { host, port, no_watch } => {
            let hub = Arc::new(open(cli.home)?);
            serve::run(hub, &host, port, !no_watch)?;
        }
        Cmd::Detect => {
            let hub = open(cli.home)?;
            for s in hub.detect_sources() {
                println!(
                    "{:<12} {:<28} {:<10} {:<9} {}",
                    if s.installed { "installed" } else { "-" },
                    s.label,
                    if s.enabled { "enabled" } else { "disabled" },
                    if s.builtin { "builtin" } else { "generic" },
                    s.root.unwrap_or_default()
                );
            }
        }
        Cmd::Search { query, agent, limit } => {
            let hub = open(cli.home)?;
            let f = EntryFilter { agent, limit: Some(limit), ..Default::default() };
            for h in hub.search(&query, &f)? {
                println!("{}  {:<12} {:<18} {}", h.id, h.agent, h.project, h.title);
                if let Some(s) = h.snippet {
                    println!("    {}", s.replace('\n', " "));
                }
            }
        }
        Cmd::List { agent, limit } => {
            let hub = open(cli.home)?;
            let f = EntryFilter { agent, limit: Some(limit), ..Default::default() };
            for h in hub.list(&f)? {
                println!("{}  {}  {:<12} {:<18} {:<16} {}", h.id, &h.updated[..10.min(h.updated.len())], h.agent, h.project, h.kind, h.title);
            }
        }
        Cmd::Show { id } => {
            let hub = open(cli.home)?;
            let e = hub.get(&id)?;
            println!("{}", memhub_core::frontmatter::render(&e.frontmatter, &e.body));
        }
        Cmd::Paths => {
            let hub = open(cli.home)?;
            println!("{}", serde_json::to_string_pretty(&hub.paths)?);
        }
        Cmd::Reindex => {
            let hub = open(cli.home)?;
            println!("indexed {} entries", hub.reindex()?);
        }
    }
    Ok(())
}

fn wait_ctrl_c() {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = ctrlc_lite::set(move || {
        let _ = tx.send(());
    });
    let _ = rx.recv();
}

/// Tiny Ctrl-C helper without an extra dependency (Unix + Windows via tokio signal).
mod ctrlc_lite {
    pub fn set<F: FnOnce() + Send + 'static>(f: F) -> std::io::Result<()> {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async {
                let _ = tokio::signal::ctrl_c().await;
            });
            f();
        });
        Ok(())
    }
}
