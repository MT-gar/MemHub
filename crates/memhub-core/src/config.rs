//! `~/.memhub/config.toml` and derived paths.

use crate::model::Kind;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_HOME_DIR: &str = ".memhub";
pub const ENV_HOME: &str = "MEMHUB_HOME";
/// Overrides the *user* home directory that agent adapters scan (`~/.claude`, `~/.codex`, ...).
/// Useful for demos and tests; on Windows `HOME` is ignored by the OS, so this is the only
/// reliable way to point MemHub at a fake home.
pub const ENV_USER_HOME: &str = "MEMHUB_USER_HOME";

/// The user's home directory: `$MEMHUB_USER_HOME` if set, otherwise the OS home.
pub fn user_home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os(ENV_USER_HOME) {
        if !h.is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    dirs::home_dir()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Vault directory (may contain `~`).
    pub vault: String,
    pub language: String,
    pub git_snapshot: bool,
    pub max_file_size_kb: u64,
    pub redact_secrets: bool,
    /// Context budget (bytes) inlined into a summary task before falling back to id lists.
    pub task_context_budget_kb: u64,
    pub sources: Vec<SourceConfig>,
    pub projects: Vec<ProjectConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            vault: "~/.memhub/vault".into(),
            language: "zh-CN".into(),
            git_snapshot: true,
            max_file_size_kb: 2048,
            redact_secrets: true,
            task_context_budget_kb: 200,
            sources: crate::adapters::BUILTIN
                .iter()
                .map(|a| SourceConfig {
                    r#type: a.id.to_string(),
                    // `#[derive(Default)]` would give `false`; a fresh install must mirror
                    // whatever `detect` finds (uninstalled agents are skipped anyway).
                    enabled: true,
                    ..Default::default()
                })
                .collect(),
            projects: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SourceConfig {
    /// Adapter id: claude-code | codex | gemini | openclaw | windsurf | generic
    pub r#type: String,
    /// Display / agent name (required for `generic`).
    pub name: Option<String>,
    /// Root directory; `None` = adapter default (auto-detect).
    pub root: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Glob patterns relative to root (generic only). Default `**/*.md`.
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// Kind for generic sources (default memory).
    pub kind: Option<Kind>,
}

fn default_true() -> bool {
    true
}

impl SourceConfig {
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.r#type.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectConfig {
    pub path: String,
    pub name: Option<String>,
}

/// Resolved filesystem locations.
#[derive(Debug, Clone, Serialize)]
pub struct Paths {
    pub home: PathBuf,
    pub config: PathBuf,
    pub vault: PathBuf,
    pub index: PathBuf,
    pub tasks: PathBuf,
    pub templates: PathBuf,
    pub logs: PathBuf,
}

impl Paths {
    /// `$MEMHUB_HOME` or `~/.memhub`.
    pub fn default_home() -> PathBuf {
        if let Ok(h) = std::env::var(ENV_HOME) {
            if !h.trim().is_empty() {
                return expand_tilde(&h);
            }
        }
        user_home()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(DEFAULT_HOME_DIR)
    }

    pub fn from_home(home: PathBuf, cfg: &Config) -> Paths {
        let home = native(home);
        let vault = if cfg.vault == "~/.memhub/vault" {
            // keep vault inside the (possibly overridden) home
            home.join("vault")
        } else {
            expand_tilde(&cfg.vault)
        };
        Paths {
            config: home.join("config.toml"),
            index: home.join("index.sqlite"),
            tasks: home.join("tasks"),
            templates: home.join("templates"),
            logs: home.join("logs"),
            vault,
            home,
        }
    }
}

pub fn expand_tilde(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(h) = user_home() {
            return native(h.join(rest));
        }
    } else if s == "~" {
        if let Some(h) = user_home() {
            return h;
        }
    }
    native(PathBuf::from(s))
}

/// Use the platform's native separator throughout, so paths coming from env vars / config
/// written with forward slashes (common in Git Bash on Windows) don't end up mixed.
#[cfg(windows)]
pub fn native(p: PathBuf) -> PathBuf {
    PathBuf::from(p.to_string_lossy().replace('/', "\\"))
}

#[cfg(not(windows))]
pub fn native(p: PathBuf) -> PathBuf {
    p
}

/// Render a path with the home directory collapsed to `~` (display only).
pub fn collapse_tilde(p: &Path) -> String {
    if let Some(h) = user_home() {
        if let Ok(rest) = p.strip_prefix(&h) {
            let r = rest.to_string_lossy();
            return if r.is_empty() {
                "~".into()
            } else {
                format!("~/{}", r.replace('\\', "/"))
            };
        }
    }
    p.to_string_lossy().replace('\\', "/")
}

impl Config {
    pub fn load(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read config {}", path.display()))?;
        let cfg: Config = toml::from_str(&text).context("parse config.toml")?;
        Ok(cfg)
    }

    pub fn load_or_create(path: &Path) -> Result<Config> {
        if path.exists() {
            Config::load(path)
        } else {
            let cfg = Config::default();
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).context("serialize config")?;
        std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }
}
