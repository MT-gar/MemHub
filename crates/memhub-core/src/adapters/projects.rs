//! Project-scoped files inside directories the user registered in `[[projects]]`.
//!
//! These are the instruction/memory files that agents look for in a repo:
//! CLAUDE.md, AGENTS.md, GEMINI.md, .cursor/rules, memory-bank/, copilot-instructions...

use super::util::{project_slug, slugify, walk};
use super::RawItem;
use crate::config::{expand_tilde, Config};
use crate::model::Kind;
use std::path::Path;

struct Rule {
    agent: &'static str,
    source: &'static str,
    globs: &'static [&'static str],
    kind: Kind,
    dest_dir: &'static str,
}

const RULES: &[Rule] = &[
    Rule { agent: "claude-code", source: "claude-code/project-instructions", globs: &["CLAUDE.md", "CLAUDE.local.md", ".claude/CLAUDE.md", ".claude/rules/**/*.md"], kind: Kind::Instruction, dest_dir: "" },
    Rule { agent: "claude-code", source: "claude-code/project-agent-memory", globs: &[".claude/agent-memory/**/*.md", ".claude/agent-memory-local/**/*.md"], kind: Kind::Memory, dest_dir: "" },
    Rule { agent: "agents-md", source: "agents-md/project", globs: &["AGENTS.md", "**/AGENTS.md"], kind: Kind::Instruction, dest_dir: "" },
    Rule { agent: "gemini", source: "gemini/project-instructions", globs: &["GEMINI.md"], kind: Kind::Instruction, dest_dir: "" },
    Rule { agent: "cursor", source: "cursor/rules", globs: &[".cursorrules", ".cursor/rules/**/*.mdc", ".cursor/rules/**/*.md"], kind: Kind::Instruction, dest_dir: "" },
    Rule { agent: "cline", source: "cline/memory-bank", globs: &["memory-bank/**/*.md"], kind: Kind::Memory, dest_dir: "" },
    Rule { agent: "copilot", source: "copilot/instructions", globs: &[".github/copilot-instructions.md", ".github/instructions/**/*.md"], kind: Kind::Instruction, dest_dir: "" },
    Rule { agent: "windsurf", source: "windsurf/project-rules", globs: &[".windsurfrules", ".windsurf/rules/**/*.md"], kind: Kind::Instruction, dest_dir: "" },
    Rule { agent: "kiro", source: "kiro/steering", globs: &[".kiro/steering/**/*.md"], kind: Kind::Instruction, dest_dir: "" },
];

pub fn collect(config: &Config) -> Vec<RawItem> {
    let mut items = Vec::new();
    for p in &config.projects {
        let root = expand_tilde(&p.path);
        if !root.is_dir() {
            continue;
        }
        let slug = p
            .name
            .as_deref()
            .map(slugify)
            .unwrap_or_else(|| project_slug(&root));
        for rule in RULES {
            let globs: Vec<String> = rule.globs.iter().map(|g| g.to_string()).collect();
            // AGENTS.md may be nested; everything else is shallow.
            let depth = if rule.globs.iter().any(|g| g.starts_with("**")) { 6 } else { 4 };
            for rel in walk(&root, &globs, &[], depth) {
                items.push(RawItem {
                    agent: rule.agent.into(),
                    source: rule.source.into(),
                    origin: root.join(&rel),
                    project: slug.clone(),
                    project_path: Some(root.clone()),
                    kind: rule.kind,
                    vault_rel: Path::new(rule.agent).join(&slug).join(rule.dest_dir).join(&rel),
                });
            }
        }
    }
    items
}
