//! Optional git snapshots of the vault (history for free, sync via remotes).

use std::path::Path;
use std::process::Command;

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Commit the current vault state. Returns `Ok(true)` if a commit was created.
pub fn snapshot(vault: &Path, message: &str) -> anyhow::Result<bool> {
    if !git_available() {
        return Ok(false);
    }
    if !vault.join(".git").exists() {
        let st = Command::new("git").arg("init").arg("-q").current_dir(vault).status()?;
        if !st.success() {
            return Ok(false);
        }
        let _ = std::fs::write(vault.join(".gitignore"), "*.tmp\n.DS_Store\n");
    }
    let add = Command::new("git").args(["add", "-A"]).current_dir(vault).status()?;
    if !add.success() {
        return Ok(false);
    }
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(vault)
        .output()?;
    if dirty.stdout.is_empty() {
        return Ok(false);
    }
    let st = Command::new("git")
        .args([
            "-c", "user.name=MemHub",
            "-c", "user.email=memhub@localhost",
            "commit", "-q", "-m", message,
        ])
        .current_dir(vault)
        .status()?;
    Ok(st.success())
}
