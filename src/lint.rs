//! Native support for the rule subset exercised by the upstream extension.
//! When @commitlint/cli exists in a project, its resolved config and full lint
//! engine are used for complete JS/TS presets/plugins/parser compatibility.
use crate::message::CommitMessage;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}, process::{Command, Stdio}, io::Write};

pub struct Commitlint {
    repo: PathBuf,
    rules: BTreeMap<String, Value>,
    type_hints: Vec<String>,
    scope_hints: Vec<String>,
    executable: Option<PathBuf>,
}

fn local_binary(repo: &Path) -> Option<PathBuf> {
    let local = repo.join("node_modules/.bin/commitlint");
    if local.is_file() { return Some(local); }
    let local_cmd = repo.join("node_modules/.bin/commitlint.cmd");
    if local_cmd.is_file() { return Some(local_cmd); }
    None
}

fn config_file(repo: &Path) -> Option<PathBuf> {
    let names = [
        ".commitlintrc", ".commitlintrc.json", ".commitlintrc.yaml", ".commitlintrc.yml",
        ".commitlintrc.js", ".commitlintrc.cjs", ".commitlintrc.mjs", ".commitlintrc.ts",
        ".commitlintrc.cts", ".commitlintrc.mts", "commitlint.config.js",
        "commitlint.config.cjs", "commitlint.config.mjs", "commitlint.config.ts",
        "commitlint.config.cts", "commitlint.config.mts", "commitlint.config.json",
        "package.json",
    ];
    names.iter().map(|name| repo.join(name)).find(|path| {
        if !path.is_file() { return false; }
        if path.file_name().is_some_and(|f| f == "package.json") {
            return fs::read_to_string(path).ok()
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .is_some_and(|v| v.get("commitlint").is_some());
        }
        true
    })
}

fn is_dynamic(path: &Path) -> bool {
    matches!(path.extension().and_then(|s| s.to_str()), Some("js" | "cjs" | "mjs" | "ts" | "cts" | "mts"))
}

fn load_native(path: &Path) -> Result<Value> {
    let source = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut v: Value = if path.extension().and_then(|x| x.to_str()) == Some("json") || path.file_name().is_some_and(|f| f == "package.json") {
        serde_json::from_str(&source).with_context(|| format!("parsing JSON {}", path.display()))?
    } else {
        serde_yaml::from_str(&source).with_context(|| format!("parsing YAML {}", path.display()))?
    };
    if path.file_name().is_some_and(|f| f == "package.json") {
        v = v.get("commitlint").cloned().ok_or_else(|| anyhow!("package.json has no commitlint config"))?;
    }
    Ok(v)
}

fn hint_keys(value: &Value, name: &str) -> Vec<String> {
    value.pointer(&format!("/prompt/questions/{name}/enum"))
        .and_then(Value::as_object).map(|x| x.keys().cloned().collect())
        .unwrap_or_default()
}

impl Commitlint {
    pub fn load(repo: &Path) -> Result<Self> {
        let config = config_file(repo);
        let binary = if config.is_some() { local_binary(repo) } else { None };
        let value = if let (Some(bin), Some(_)) = (&binary, &config) {
            let output = Command::new(bin).current_dir(repo).args(["--print-config", "json"])
                .output().with_context(|| format!("running {} --print-config json", bin.display()))?;
            if !output.status.success() {
                bail!("commitlint config load failed: {}", String::from_utf8_lossy(&output.stderr).trim());
            }
            serde_json::from_slice::<Value>(&output.stdout)
                .context("commitlint --print-config json returned invalid JSON")?
        } else if let Some(path) = &config {
            if is_dynamic(path) {
                bail!("{} requires the project to install @commitlint/cli. Run 'npm install -D @commitlint/cli' to resolve JS/TS config; this Rust tool never executes JS directly.", path.display());
            }
            let v = load_native(path)?;
            if v.get("extends").and_then(Value::as_array).is_some_and(|xs| !xs.is_empty()) {
                bail!("{} uses commitlint 'extends'; install @commitlint/cli in the project so presets can be resolved safely", path.display());
            }
            v
        } else { Value::Null };

        let rules = value.get("rules").and_then(Value::as_object)
            .map(|r| r.iter().map(|(k,v)| (k.to_owned(), v.clone())).collect())
            .unwrap_or_default();
        let result = Self {
            repo: repo.to_path_buf(),
            rules,
            type_hints: hint_keys(&value, "type"),
            scope_hints: hint_keys(&value, "scope"),
            executable: binary,
        };
        // Do not silently skip nontrivial upstream rules without the official CLI.
        if result.executable.is_none() {
            for (key, rule) in &result.rules {
                if severity(rule) == 2 && !SUPPORTED_RULES.contains(&key.as_str()) {
                    bail!("Rule '{key}' needs @commitlint/cli for full validation; install it locally to enable this config");
                }
            }
        }
        Ok(result)
    }

    pub fn types(&self, fallback: &[String]) -> Vec<String> {
        let allowed = self.enum_values("type-enum");
        if !allowed.is_empty() { return allowed; }
        let mut v = fallback.to_vec();
        for t in &self.type_hints { if !v.contains(t) { v.push(t.clone()); } }
        v
    }

    pub fn scopes(&self, fallback: &[String]) -> Vec<String> {
        if !self.scope_hints.is_empty() { return self.scope_hints.clone(); }
        let allowed = self.enum_values("scope-enum");
        if !allowed.is_empty() { return allowed; }
        fallback.to_vec()
    }

    fn enum_values(&self, rule_name: &str) -> Vec<String> {
        let Some(rule) = self.rules.get(rule_name).and_then(Value::as_array) else { return Vec::new(); };
        if rule.first().and_then(Value::as_i64) != Some(2) || rule.get(1).and_then(Value::as_str) == Some("never") {
            return Vec::new();
        }
        rule.get(2).and_then(Value::as_array).map(|list| list.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default()
    }

    pub fn validate(&self, msg: &CommitMessage) -> Result<()> {
        if msg.kind.is_empty() { bail!("type must not be empty"); }
        if msg.subject.is_empty() { bail!("subject must not be empty"); }
        if msg.kind.chars().any(|c| matches!(c, '\n' | '\r' | ':')) || msg.scope.chars().any(|c| matches!(c, '\n' | '\r' | '(' | ')')) {
            bail!("invalid newline or delimiter in type/scope");
        }
        if msg.subject.chars().any(|c| matches!(c, '\n' | '\r')) { bail!("subject cannot contain a newline"); }
        let header = msg.header();
        let subject_line = msg.subject_line();
        for (name, value) in [
            ("type", msg.kind.as_str()), ("scope", msg.scope.as_str()),
            ("subject", subject_line.as_str()), ("header", header.as_str()),
            ("body", msg.body.as_str()), ("footer", msg.footer.as_str()),
        ] {
            self.check_field(name, value)?;
        }
        if let Some(bin) = &self.executable {
            let mut child = Command::new(bin).current_dir(&self.repo).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
                .spawn().context("running @commitlint/cli for full validation")?;
            child.stdin.take().context("commitlint stdin")?.write_all(msg.serialize().as_bytes())?;
            let output = child.wait_with_output()?;
            if !output.status.success() {
                bail!("commitlint validation failed:\n{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            }
        }
        Ok(())
    }

    fn check_field(&self, name: &str, value: &str) -> Result<()> {
        for (key, rule) in &self.rules {
            if !key.starts_with(&format!("{name}-")) || severity(rule) != 2 { continue; }
            let array = rule.as_array().ok_or_else(|| anyhow!("malformed commitlint rule '{key}'"))?;
            let mode = array.get(1).and_then(Value::as_str).unwrap_or("always");
            let condition = array.get(2).unwrap_or(&Value::Null);
            let action = key.strip_prefix(&format!("{name}-")).unwrap_or_default();
            let (valid, detail) = match action {
                "enum" => {
                    let opts: Vec<&str> = condition.as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                    // commitlint usually skips enum validation when a scope is empty.
                    (value.is_empty() || opts.contains(&value), format!("must be one of {opts:?}"))
                }
                "empty" => (value.is_empty(), "must be empty".to_string()),
                "full-stop" => (value.ends_with(condition.as_str().unwrap_or(".")), "full-stop rule failed".to_string()),
                "max-length" => (value.chars().count() <= condition.as_u64().unwrap_or(u64::MAX) as usize, "too long".to_string()),
                "min-length" => (value.chars().count() >= condition.as_u64().unwrap_or(0) as usize, "too short".to_string()),
                "case" => (matches_case(value, condition), format!("invalid case; required {condition}")),
                _ => continue,
            };
            let ok = if mode == "never" { !valid } else { valid };
            if !ok { bail!("{key}: {detail} ({value:?})"); }
        }
        Ok(())
    }
}

const SUPPORTED_RULES: &[&str] = &[
    "type-enum", "scope-enum", "type-case", "scope-case", "subject-case", "header-case",
    "type-empty", "scope-empty", "subject-empty", "header-empty", "body-empty", "footer-empty",
    "type-min-length", "type-max-length", "scope-min-length", "scope-max-length",
    "subject-min-length", "subject-max-length", "header-min-length", "header-max-length",
    "body-min-length", "body-max-length", "footer-min-length", "footer-max-length",
    "subject-full-stop", "header-full-stop", "body-full-stop",
];

fn severity(rule: &Value) -> i64 { rule.as_array().and_then(|v| v.first()).and_then(Value::as_i64).unwrap_or(0) }

fn matches_case(value: &str, condition: &Value) -> bool {
    let cases: Vec<&str> = if let Some(s) = condition.as_str() { vec![s] }
        else { condition.as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default() };
    if value.is_empty() { return true; }
    cases.iter().any(|case| match *case {
        "lower-case" => value == value.to_lowercase(),
        "upper-case" => value == value.to_uppercase(),
        "sentence-case" => value.chars().next().is_some_and(|c| c.is_uppercase()),
        "start-case" | "pascal-case" => value.chars().next().is_some_and(|c| c.is_uppercase()),
        "kebab-case" => value == value.to_lowercase() && !value.contains(' ') && !value.contains('_'),
        "snake-case" => value == value.to_lowercase() && !value.contains(' ') && !value.contains('-'),
        "camel-case" => value.chars().next().is_some_and(|c| c.is_lowercase()) && !value.contains(' ') && !value.contains('_') && !value.contains('-'),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_json_type_enum_and_rejects_nonmatching_type() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join(".commitlintrc.json"), r#"{"rules":{"type-enum":[2,"always",["special"]],"header-max-length":[2,"always",72]}}"#).unwrap();
        let lint = Commitlint::load(temp.path()).unwrap();
        assert_eq!(lint.types(&[]), vec!["special"]);
        let mut m = CommitMessage { kind: "feat".into(), subject: "hello".into(), ..Default::default() };
        assert!(lint.validate(&m).is_err());
        m.kind = "special".into();
        assert!(lint.validate(&m).is_ok());
    }

    #[test]
    fn reads_yaml_scope_enum() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join(".commitlintrc.yaml"), "rules:\n  scope-enum: [2, always, [api, web]]\n").unwrap();
        let lint = Commitlint::load(temp.path()).unwrap();
        assert_eq!(lint.scopes(&[]), vec!["api", "web"]);
    }

    #[test]
    fn rejects_unknown_enforced_rule_without_commitlint() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join(".commitlintrc.json"), r#"{"rules":{"references-empty":[2,"never"]}}"#).unwrap();
        assert!(Commitlint::load(temp.path()).is_err());
    }
}
