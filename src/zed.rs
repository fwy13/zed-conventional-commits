use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{env, fs, path::{Path, PathBuf}};

fn zed_config_root() -> Result<PathBuf> {
    #[cfg(windows)]
    if let Some(p) = env::var_os("APPDATA") { return Ok(PathBuf::from(p).join("Zed")); }
    if let Some(p) = env::var_os("XDG_CONFIG_HOME") { return Ok(PathBuf::from(p).join("zed")); }
    if let Some(p) = env::var_os("HOME") { return Ok(PathBuf::from(p).join(".config/zed")); }
    bail!("Cannot locate Zed config directory: set XDG_CONFIG_HOME or HOME")
}

#[cfg(test)]
pub fn install_tasks(repo: &Path, global: bool) -> Result<PathBuf> {
    install_tasks_with_bin(repo, global, None)
}

pub fn install_tasks_with_bin(repo: &Path, global: bool, bin: Option<&Path>) -> Result<PathBuf> {
    let binary = if let Some(path) = bin {
        if !path.is_absolute() { bail!("--bin must be an absolute path"); }
        path.to_str().context("--bin path must be valid UTF-8")?.to_owned()
    } else {
        "zed-cc".to_owned()
    };
    let path = if global { zed_config_root()?.join("tasks.json") } else { repo.join(".zed/tasks.json") };
    let mut tasks: Vec<Value> = if path.exists() {
        let text = fs::read_to_string(&path).context("reading existing tasks.json")?;
        // Never overwrite JSONC files that cannot be safely parsed as JSON.
        serde_json::from_str(&text).with_context(|| format!("{} contains JSONC or invalid JSON: please add tasks manually using examples/tasks.json; your file was not changed", path.display()))?
    } else { Vec::new() };
    let new_tasks = vec![
        json!({
            "label": "Conventional Commit (Rust)",
            "command": binary.clone(),
            "args": ["commit", "--repo", "$ZED_WORKTREE_ROOT"],
            "reveal": "always", "hide": "never", "save": "all"
        }),
        json!({
            "label": "Conventional Commit: Preview (Rust)",
            "command": binary.clone(),
            "args": ["commit", "--repo", "$ZED_WORKTREE_ROOT", "--dry-run"],
            "reveal": "always", "hide": "never", "save": "all"
        })
    ];
    for new_task in new_tasks {
        let label = new_task["label"].as_str();
        if let Some(i) = tasks.iter().position(|t| t["label"].as_str() == label) {
            tasks[i] = new_task;
        } else {
            tasks.push(new_task);
        }
    }
    let parent = path.parent().context("invalid Zed tasks path")?;
    fs::create_dir_all(parent)?;
    if path.exists() {
        fs::copy(&path, path.with_extension("json.bak"))
            .with_context(|| format!("backing up {}", path.display()))?;
    }
    fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&tasks)?))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserve_existing_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let zed = dir.path().join(".zed");
        fs::create_dir_all(&zed).unwrap();
        fs::write(zed.join("tasks.json"), "[{\"label\":\"other\",\"command\":\"echo\"}]").unwrap();
        install_tasks(dir.path(), false).unwrap();
        let tasks: Vec<Value> = serde_json::from_str(&fs::read_to_string(zed.join("tasks.json")).unwrap()).unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0]["label"], "other");
        assert!(zed.join("tasks.json.bak").exists());
        install_tasks(dir.path(), false).unwrap();
        let tasks: Vec<Value> = serde_json::from_str(&fs::read_to_string(zed.join("tasks.json")).unwrap()).unwrap();
        assert_eq!(tasks.len(), 3);
    }

    #[test]
    fn custom_absolute_binary_path_is_written_to_both_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let binary = if cfg!(windows) { Path::new("C:\\Program Files\\zed-cc.exe") } else { Path::new("/opt/zed cc/zed-cc") };
        install_tasks_with_bin(dir.path(), false, Some(binary)).unwrap();
        let tasks: Vec<Value> = serde_json::from_str(
            &fs::read_to_string(dir.path().join(".zed/tasks.json")).unwrap()
        ).unwrap();
        assert_eq!(tasks.len(), 2);
        for task in tasks {
            assert_eq!(task["command"], binary.to_str().unwrap());
        }
    }

    #[test]
    fn relative_binary_path_is_rejected_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(install_tasks_with_bin(dir.path(), false, Some(Path::new("./zed-cc"))).is_err());
        assert!(!dir.path().join(".zed/tasks.json").exists());
    }
}
