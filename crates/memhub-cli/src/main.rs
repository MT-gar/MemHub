mod guard;
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
        /// Also accept requests from this Origin (repeatable), e.g. a dev server
        #[arg(long = "allow-origin")]
        allow_origin: Vec<String>,
        /// Require this access token (auto-generated when --host is not a loopback address)
        #[arg(long, env = "MEMHUB_TOKEN")]
        token: Option<String>,
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
    /// Print the approved rules for a project (for session hooks / `AGENTS.md` includes)
    Context {
        /// Project directory or slug (default: the current directory)
        #[arg(long)]
        project: Option<String>,
        /// Only the global rules, ignore the current directory
        #[arg(long)]
        global: bool,
        /// md | json
        #[arg(long, default_value = "md")]
        format: String,
        /// Maximum number of rules
        #[arg(long, default_value_t = 40)]
        max_lines: usize,
        /// Append a short id to every rule (to refer to it in `memhub rules`)
        #[arg(long)]
        with_ids: bool,
    },
    /// Review, approve and retire rules
    Rules {
        #[command(subcommand)]
        cmd: RulesCmd,
    },
}

#[derive(Subcommand)]
enum RulesCmd {
    /// List rules (newest last)
    List {
        /// draft | approved | retired
        #[arg(long)]
        status: Option<String>,
        /// global | project:<name>
        #[arg(long)]
        scope: Option<String>,
    },
    /// Add a rule (saved as a draft unless --approve)
    Add {
        /// One short sentence, e.g. "Use pnpm, not npm"
        text: String,
        /// global (default) or project:<name>
        #[arg(long)]
        scope: Option<String>,
        /// Why (shown during review, never served to agents)
        #[arg(long)]
        detail: Option<String>,
        /// Approve immediately (needs an interactive terminal or --yes)
        #[arg(long)]
        approve: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Approve a draft or retired rule so agents receive it
    Approve {
        id: String,
        /// Skip the interactive-terminal check (scripts you wrote yourself)
        #[arg(long)]
        yes: bool,
    },
    /// Stop serving a rule (kept for history)
    Retire { id: String },
    /// Move a rule back to draft
    Draft { id: String },
    /// Delete a rule file for good
    Rm { id: String },
}

/// Approval is a human decision: refuse when stdin is not a terminal unless `--yes` is given,
/// so an agent running shell commands does not approve its own proposals by accident.
fn require_human(yes: bool, what: &str) -> Result<()> {
    use std::io::IsTerminal;
    if yes || std::io::stdin().is_terminal() {
        return Ok(());
    }
    anyhow::bail!("{what} must be confirmed by a person: run it in your own terminal, use the MemHub app, or pass --yes if you really mean it")
}

fn print_rules(rules: &[memhub_core::rules::Rule]) {
    if rules.is_empty() {
        println!("(no rules)");
    }
    for r in rules {
        let id = &r.id[r.id.len().saturating_sub(6)..];
        let warn = if r.warnings.is_empty() { String::new() } else { format!("  ⚠ {}", r.warnings.join("; ")) };
        println!("{id}  {:<9} {:<18} {}{warn}", r.status.as_str(), r.scope, r.text);
    }
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
        Cmd::Serve { host, port, no_watch, allow_origin, token } => {
            let hub = Arc::new(open(cli.home)?);
            serve::run(hub, serve::Options { host, port, watch: !no_watch, allow_origins: allow_origin, token })?;
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
        Cmd::Context { project, global, format, max_lines, with_ids } => {
            let hub = open(cli.home)?;
            let project = if global { None } else { Some(project.unwrap_or_else(|| ".".into())) };
            let opts = memhub_core::rules::CompileOpts { max_lines: max_lines.clamp(1, 500), with_ids, ..Default::default() };
            let c = memhub_core::rules::compile(&hub, project.as_deref(), &opts)?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&c)?);
            } else {
                // Nothing approved → print nothing, so hooks inject nothing.
                print!("{}", c.markdown);
            }
        }
        Cmd::Rules { cmd } => {
            use memhub_core::rules::{self, NewRule, Status};
            let hub = open(cli.home)?;
            match cmd {
                RulesCmd::List { status, scope } => {
                    let st = match status.as_deref() {
                        Some(s) => Some(Status::parse(s).ok_or_else(|| anyhow::anyhow!("status must be draft | approved | retired"))?),
                        None => None,
                    };
                    print_rules(&rules::list(&hub, scope.as_deref(), st)?);
                }
                RulesCmd::Add { text, scope, detail, approve, yes } => {
                    if approve {
                        require_human(yes, "approving a rule")?;
                    }
                    let st = if approve { Status::Approved } else { Status::Draft };
                    let o = rules::add(&hub, NewRule { text, detail, scope, status: Some(st), agent: "user".into(), ..Default::default() })?;
                    println!("{} {}", if o.created { "added" } else { "already exists:" }, &o.rule.id);
                    print_rules(&[o.rule]);
                }
                RulesCmd::Approve { id, yes } => {
                    require_human(yes, "approving a rule")?;
                    print_rules(&[rules::set_status(&hub, &id, Status::Approved)?]);
                }
                RulesCmd::Retire { id } => print_rules(&[rules::set_status(&hub, &id, Status::Retired)?]),
                RulesCmd::Draft { id } => print_rules(&[rules::set_status(&hub, &id, Status::Draft)?]),
                RulesCmd::Rm { id } => {
                    let r = rules::get(&hub, &id)?;
                    hub.delete_entry(&r.id)?;
                    println!("deleted {}", r.text);
                }
            }
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
