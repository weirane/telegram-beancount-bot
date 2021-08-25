use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, ensure, Context, Result};

pub fn check_repo(repo: &str) -> Result<()> {
    let out = Command::new("git")
        .args(&["-C", repo, "pull", "--rebase"])
        .output()
        .context("execution of git pull --rebase failed")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(anyhow!("git pull --rebase failed").context(stderr));
    }

    Ok(())
}

pub fn commit_file(repo: &str, file: &Path, orig_cmd: Option<&str>) -> Result<()> {
    // TODO: capture error message
    let st = Command::new("git")
        .args(&["-C", repo, "add"])
        .arg(file)
        .status()?;
    ensure!(st.success(), "git add failed");

    let mut cmd = &mut Command::new("git");
    cmd = cmd.args(&["-C", repo, "commit", "-m", "Add a transaction"]);
    if let Some(orig_cmd) = orig_cmd {
        cmd = cmd.args(&["-m", orig_cmd]);
    }
    let st = cmd.status()?;
    ensure!(st.success(), "git commit failed");

    let st = Command::new("git").args(&["-C", repo, "push"]).status()?;
    ensure!(st.success(), "git push failed");
    Ok(())
}
