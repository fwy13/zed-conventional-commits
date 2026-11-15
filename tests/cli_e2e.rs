use std::{fs, path::Path, process::{Command, Output}};
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) -> Output {
    Command::new("git").arg("-C").arg(root).args(args).output().expect("git")
}

fn create_repo() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    assert!(git(root.path(), &["init"]).status.success());
    assert!(git(root.path(), &["config", "user.name", "Test User"]).status.success());
    assert!(git(root.path(), &["config", "user.email", "test@example.com"]).status.success());
    assert!(git(root.path(), &["config", "commit.gpgsign", "false"]).status.success());
    fs::write(root.path().join("README.md"), "initial\n").unwrap();
    assert!(git(root.path(), &["add", "README.md"]).status.success());
    assert!(git(root.path(), &["commit", "-m", "chore: initial"]).status.success());
    root
}

fn exe(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zed-cc")).args(args).output().unwrap()
}

#[test]
fn composes_using_the_original_serialization() {
    let result = exe(&["compose", "--type", "feat", "--scope", "api", "--gitmoji", ":sparkles:",
        "--subject", "add login", "--tag", "[release]", "--skip-ci", "--body", "details",
        "--footer", "BREAKING CHANGE: routes"]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim_end(),
        "feat(api): :sparkles: add login [release] [skip ci]\n\ndetails\n\nBREAKING CHANGE: routes");
}

#[test]
fn commit_only_staged_files_and_does_not_add_untracked() {
    let dir = create_repo();
    fs::write(dir.path().join("README.md"), "changed\n").unwrap();
    fs::write(dir.path().join("secret.txt"), "do not stage\n").unwrap();
    assert!(git(dir.path(), &["add", "README.md"]).status.success());
    let repo = dir.path().to_str().unwrap();
    let result = exe(&["commit", "--repo", repo, "--type", "fix", "--subject", "update readme"]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let log = git(dir.path(), &["log", "-1", "--pretty=%B"]);
    assert_eq!(String::from_utf8_lossy(&log.stdout).trim(), "fix: update readme");
    assert_eq!(git(dir.path(), &["ls-files", "secret.txt"]).stdout.len(), 0);
}

#[test]
fn dry_run_does_not_modify_index_or_head() {
    let dir = create_repo();
    fs::write(dir.path().join("README.md"), "changed\n").unwrap();
    let repo = dir.path().to_str().unwrap();
    let before = git(dir.path(), &["rev-parse", "HEAD"]).stdout;
    let result = exe(&["commit", "--repo", repo, "--type", "feat", "--subject", "preview", "--dry-run", "--all"]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(before, git(dir.path(), &["rev-parse", "HEAD"]).stdout);
    assert!(git(dir.path(), &["diff", "--cached", "--name-only"]).stdout.is_empty());
}

#[test]
fn refuses_commit_if_index_is_empty() {
    let dir = create_repo();
    let result = exe(&["commit", "--repo", dir.path().to_str().unwrap(), "--type", "fix", "--subject", "nothing"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("no staged changes"));
}

#[test]
fn reads_commitlint_rules_in_repo() {
    let dir = create_repo();
    fs::write(dir.path().join(".commitlintrc.json"),
        r#"{"rules":{"type-enum":[2,"always",["special"]]}}"#).unwrap();
    let repo = dir.path().to_str().unwrap();
    let bad = exe(&["compose", "--repo", repo, "--type", "feat", "--subject", "not allowed"]);
    assert!(!bad.status.success());
    let good = exe(&["compose", "--repo", repo, "--type", "special", "--subject", "allowed"]);
    assert!(good.status.success(), "{}", String::from_utf8_lossy(&good.stderr));
}

#[test]
fn init_zed_merges_tasks_and_keeps_existing() {
    let dir = create_repo();
    fs::create_dir_all(dir.path().join(".zed")).unwrap();
    fs::write(dir.path().join(".zed/tasks.json"), r#"[{"label":"my build","command":"cargo build"}]"#).unwrap();
    let result = exe(&["init-zed", "--repo", dir.path().to_str().unwrap()]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let file = fs::read_to_string(dir.path().join(".zed/tasks.json")).unwrap();
    let tasks: Vec<serde_json::Value> = serde_json::from_str(&file).unwrap();
    assert_eq!(tasks.len(), 3);
    assert!(tasks.iter().any(|task| task["label"] == "my build"));
}

#[test]
fn runs_from_git_repo_with_spaces_in_path() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repository with spaces");
    fs::create_dir_all(&repo).unwrap();
    assert!(git(&repo, &["init"]).status.success());
    assert!(git(&repo, &["config", "user.name", "Test User"]).status.success());
    assert!(git(&repo, &["config", "user.email", "test@example.com"]).status.success());
    assert!(git(&repo, &["config", "commit.gpgsign", "false"]).status.success());
    fs::write(repo.join("README.md"), "with spaces\n").unwrap();
    let result = exe(&["commit", "--repo", repo.to_str().unwrap(), "--type", "docs",
        "--subject", "support spaces", "--all"]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(String::from_utf8_lossy(&git(&repo, &["log", "-1", "--pretty=%s"]).stdout).contains("docs: support spaces"));
}

#[test]
fn init_zed_with_absolute_binary_supports_distribution_without_path() {
    let dir = create_repo();
    let binary = std::env::current_exe().unwrap();
    let result = exe(&["init-zed", "--repo", dir.path().to_str().unwrap(),
        "--bin", binary.to_str().unwrap()]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let tasks: Vec<serde_json::Value> = serde_json::from_str(
        &fs::read_to_string(dir.path().join(".zed/tasks.json")).unwrap()
    ).unwrap();
    assert!(tasks.iter().all(|task| task["command"] == binary.to_str().unwrap()));
}
