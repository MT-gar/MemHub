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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectConfig;
    use crate::testutil::TempDir;

    #[test]
    fn registered_project_instruction_files_are_found_once() {
        let t = TempDir::new("projects");
        t.write("CLAUDE.md", "# c\n");
        t.write("AGENTS.md", "# a\n");
        t.write("packages/web/AGENTS.md", "# nested\n");
        t.write(".cursor/rules/style.mdc", "---\nalwaysApply: true\n---\nuse tabs\n");
        t.write(".github/copilot-instructions.md", "# copilot\n");
        t.write("memory-bank/activeContext.md", "# ctx\n");
        t.write("node_modules/pkg/AGENTS.md", "# must be skipped\n");
        let cfg = Config {
            projects: vec![ProjectConfig { path: t.path().to_string_lossy().to_string(), name: Some("My App".into()) }],
            ..Default::default()
        };
        let items = collect(&cfg);
        let agents: Vec<&str> = items.iter().map(|i| i.agent.as_str()).collect();
        for want in ["claude-code", "agents-md", "cursor", "copilot", "cline"] {
            assert!(agents.contains(&want), "missing {want}: {agents:?}");
        }
        // AGENTS.md at the root and nested, but nothing from node_modules
        assert_eq!(items.iter().filter(|i| i.agent == "agents-md").count(), 2, "{items:#?}");
        assert!(items.iter().all(|i| !i.origin.to_string_lossy().contains("node_modules")));
        assert!(items.iter().all(|i| i.project == "My-App" || i.project == "my-app" || i.project.eq_ignore_ascii_case("my-app")));
        assert_eq!(items.iter().find(|i| i.agent == "cline").unwrap().kind, Kind::Memory);
        assert_eq!(items.iter().find(|i| i.agent == "cursor").unwrap().kind, Kind::Instruction);
    }

    #[test]
    fn missing_project_directory_is_ignored() {
        let cfg = Config {
            projects: vec![ProjectConfig { path: "/definitely/not/here/memhub-test".into(), name: None }],
            ..Default::default()
        };
        assert!(collect(&cfg).is_empty());
    }
}
