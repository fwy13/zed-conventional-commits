use anyhow::{bail, Context, Result};
use std::{path::{Path, PathBuf}, process::{Command, Stdio}, io::Write};

pub fn root(path: &Path) -> Result<PathBuf> {
    let output = Command::new("git").arg("-C").arg(path).args(["rev-parse", "--show-toplevel"])
        .output().context("git is not installed or cannot be executed")?;
    if !output.status.success() {
        bail!("Not a Git repository: {}\n{}", path.display(), String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()))
}

pub fn has_staged_changes(repo: &Path) -> Result<bool> {
    let status = Command::new("git").arg("-C").arg(repo)
        .args(["diff", "--cached", "--quiet", "--exit-code"])
        .status().context("checking Git index")?;
    match status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => bail!("git diff --cached failed (status {status})"),
    }
}

pub fn stage_all(repo: &Path) -> Result<()> {
    run(repo, &["add", "-A"])
}

pub fn commit(repo: &Path, message: &str, edit: bool) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).arg("commit").arg("-F").arg("-");
    if edit { cmd.arg("--edit"); }
    let mut child = cmd.stdin(Stdio::piped()).spawn().context("starting git commit")?;
    child.stdin.take().context("opening git commit stdin")?.write_all(message.as_bytes())?;
    let status = child.wait().context("waiting for git commit")?;
    if !status.success() { bail!("git commit failed with exit status {status}"); }
    Ok(())
}

pub fn push(repo: &Path) -> Result<()> { run(repo, &["push"]) }

fn run(repo: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git").arg("-C").arg(repo).args(args)
        .status().with_context(|| format!("running git {}", args.join(" ")))?;
    if !status.success() { bail!("git {} failed (status {status})", args.join(" ")); }
    Ok(())
}
