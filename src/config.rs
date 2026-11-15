use anyhow::{Context, Result};
use serde::Deserialize;
use std::{env, fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub auto_commit: bool,
    pub auto_push: bool,
    pub stage_all: bool,
    pub gitmoji: bool,
    pub emoji_format: String,
    pub line_break: String,
    pub prompt_scopes: bool,
    pub prompt_body: bool,
    pub prompt_footer: bool,
    pub prompt_ci: bool,
    pub prompt_tag: bool,
    pub scopes: Vec<String>,
    pub tags: Vec<String>,
    pub types: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_commit: true,
            auto_push: false,
            stage_all: false,
            gitmoji: true,
            emoji_format: "code".into(),
            line_break: String::new(),
            prompt_scopes: true,
            prompt_body: true,
            prompt_footer: true,
            prompt_ci: false,
            prompt_tag: false,
            scopes: vec![],
            tags: vec![],
            types: vec!["feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert"]
                .into_iter().map(str::to_owned).collect(),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PartialConfig {
    auto_commit: Option<bool>,
    auto_push: Option<bool>,
    stage_all: Option<bool>,
    gitmoji: Option<bool>,
    emoji_format: Option<String>,
    line_break: Option<String>,
    prompt_scopes: Option<bool>,
    prompt_body: Option<bool>,
    prompt_footer: Option<bool>,
    prompt_ci: Option<bool>,
    prompt_tag: Option<bool>,
    scopes: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    types: Option<Vec<String>>,
}

impl Config {
    pub fn load(repo: &Path) -> Result<Self> {
        let mut config = Self::default();
        if let Some(global) = global_config_path() {
            config.overlay(&global)?;
        }
        config.overlay(&repo.join(".zed-conventional-commits.toml"))?;
        anyhow::ensure!(matches!(config.emoji_format.as_str(), "code" | "emoji"), "emoji_format must be 'code' or 'emoji'");
        anyhow::ensure!(!config.types.is_empty(), "types must not be empty");
        Ok(config)
    }

    fn overlay(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let part: PartialConfig = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        macro_rules! set { ($($field:ident),+ $(,)?) => { $(if let Some(value) = part.$field { self.$field = value; })+ }; }
        set!(auto_commit, auto_push, stage_all, gitmoji, emoji_format, line_break,
            prompt_scopes, prompt_body, prompt_footer, prompt_ci, prompt_tag, scopes, tags, types);
        Ok(())
    }
}

pub fn global_config_path() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("zed-conventional-commits/config.toml"));
    }
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/zed-conventional-commits/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_has_source_compatible_flags() {
        let c = Config::default();
        assert!(c.auto_commit && c.gitmoji && c.prompt_scopes && c.prompt_body && c.prompt_footer);
        assert!(!c.prompt_ci && !c.prompt_tag && !c.auto_push && !c.stage_all);
        assert!(c.types.contains(&"feat".to_string()));
    }
}

/// Add an entry without losing any existing TOML settings. The original
/// extension keeps scopes and tags in workspace/global VS Code settings;
/// this port stores them in the project TOML file instead.
pub fn add_project_item(repo: &Path, key: &str, item: &str) -> Result<PathBuf> {
    anyhow::ensure!(key == "scopes" || key == "tags", "unsupported setting {key}");
    let item = item.trim();
    anyhow::ensure!(!item.is_empty(), "value cannot be empty");
    let path = repo.join(".zed-conventional-commits.toml");
    let mut val: toml::Value = if path.exists() {
        fs::read_to_string(&path)?.parse().context("parsing existing TOML config")?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };
    let table = val.as_table_mut().context("configuration root must be a TOML table")?;
    let values = table.entry(key.to_string())
        .or_insert_with(|| toml::Value::Array(Vec::new()))
        .as_array_mut().context("scopes/tags must be arrays")?;
    if !values.iter().any(|v| v.as_str() == Some(item)) {
        values.push(toml::Value::String(item.to_owned()));
    }
    fs::write(&path, toml::to_string_pretty(&val)?)?;
    Ok(path)
}
