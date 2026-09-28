//! Best-effort secret redaction applied when mirroring agent files.

use regex::Regex;
use std::sync::OnceLock;

fn patterns() -> &'static Vec<Regex> {
    static P: OnceLock<Vec<Regex>> = OnceLock::new();
    P.get_or_init(|| {
        [
            r"sk-ant-[A-Za-z0-9_\-]{20,}",          // Anthropic
            r"sk-proj-[A-Za-z0-9_\-]{20,}",         // OpenAI project keys
            r"sk-[A-Za-z0-9_\-]{20,}",              // OpenAI / generic sk-
            r"ghp_[A-Za-z0-9]{30,}",                // GitHub PAT
            r"github_pat_[A-Za-z0-9_]{30,}",
            r"gho_[A-Za-z0-9]{30,}",
            r"AKIA[0-9A-Z]{16}",                    // AWS access key id
            r"xox[baprs]-[A-Za-z0-9\-]{10,}",       // Slack
            r"AIza[0-9A-Za-z_\-]{30,}",             // Google API key
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
            r"(?i)\b(api[_\-]?key|secret|token|password|passwd)\b\s*[:=]\s*['\x22]?[A-Za-z0-9_\-\./+]{16,}['\x22]?",
        ]
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
    })
}

/// Returns the redacted text and whether anything was replaced.
pub fn redact(text: &str) -> (String, bool) {
    let mut out = text.to_string();
    let mut changed = false;
    for re in patterns() {
        if re.is_match(&out) {
            changed = true;
            out = re
                .replace_all(&out, |caps: &regex::Captures| {
                    // Keep the key name for `key = value` style matches.
                    if let Some(name) = caps.get(1) {
                        format!("{}: [REDACTED]", name.as_str())
                    } else {
                        "[REDACTED]".to_string()
                    }
                })
                .to_string();
        }
    }
    (out, changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redacts_keys() {
        let (t, c) = redact("key sk-abcdefghijklmnopqrstuvwxyz1234 and AKIAABCDEFGHIJKLMNOP");
        assert!(c);
        assert!(!t.contains("sk-abc"));
        assert!(!t.contains("AKIA"));
    }
    #[test]
    fn leaves_normal_text() {
        let (t, c) = redact("Use `cargo test` before pushing.");
        assert!(!c);
        assert_eq!(t, "Use `cargo test` before pushing.");
    }
}
