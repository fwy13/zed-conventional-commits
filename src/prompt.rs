use crate::{config::{add_project_item, Config}, lint::Commitlint, message::CommitMessage};
use anyhow::{bail, Result};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::path::Path;

const EMOJIS: &[(&str, &str, &str)] = &[
    (":sparkles:", "✨", "Introduce new features"),
    (":bug:", "🐛", "Fix a bug"),
    (":memo:", "📝", "Add or update documentation"),
    (":art:", "🎨", "Improve structure/format"),
    (":recycle:", "♻️", "Refactor code"),
    (":zap:", "⚡️", "Improve performance"),
    (":white_check_mark:", "✅", "Add or update tests"),
    (":construction_worker:", "👷", "Update CI build system"),
    (":wrench:", "🔧", "Update configuration"),
    (":fire:", "🔥", "Remove code or files"),
    (":lock:", "🔒", "Fix security issues"),
    (":arrow_up:", "⬆️", "Upgrade dependencies"),
    (":arrow_down:", "⬇️", "Downgrade dependencies"),
    (":heavy_plus_sign:", "➕", "Add a dependency"),
    (":heavy_minus_sign:", "➖", "Remove a dependency"),
    (":lipstick:", "💄", "Improve UI/style"),
    (":truck:", "🚚", "Move or rename resources"),
    (":rocket:", "🚀", "Deploy"),
    (":boom:", "💥", "Introduce breaking changes"),
    (":tada:", "🎉", "Begin project"),
    (":see_no_evil:", "🙈", "Update .gitignore"),
    (":rewind:", "⏪", "Revert changes"),
    (":ambulance:", "🚑", "Critical hotfix"),
    (":package:", "📦", "Update package/build"),
    (":poop:", "💩", "Temporary code"),
];

enum StepResult<T> { Back, Value(T) }

fn ask_text(what: &str, default: &str, optional: bool) -> Result<StepResult<String>> {
    let theme = ColorfulTheme::default();
    let value: String = Input::with_theme(&theme).with_prompt(format!("{what} (type :back to return)"))
        .with_initial_text(default).allow_empty(optional).interact_text()?;
    if value.trim() == ":back" { return Ok(StepResult::Back); }
    if !optional && value.trim().is_empty() { bail!("{what} cannot be empty"); }
    Ok(StepResult::Value(value))
}

fn choose(what: &str, options: &[String], default: Option<&str>, back: bool) -> Result<StepResult<usize>> {
    let theme = ColorfulTheme::default();
    let mut choices = options.to_vec();
    if back { choices.push("← Back".to_string()); }
    let position = default.and_then(|s| options.iter().position(|o| o == s)).unwrap_or(0);
    let answer = Select::with_theme(&theme).with_prompt(what).items(&choices)
        .default(position).interact()?;
    if back && answer == options.len() { return Ok(StepResult::Back); }
    Ok(StepResult::Value(answer))
}

fn pick_optional(repo: &Path, options: &[String], label: &str, key: &str) -> Result<StepResult<String>> {
    let mut choices = vec!["(none)".to_string()];
    choices.extend(options.iter().cloned());
    choices.extend(["(enter once)", "(enter and save to project)"].iter().map(|x| x.to_string()));
    let n = match choose(label, &choices, None, true)? {
        StepResult::Back => return Ok(StepResult::Back),
        StepResult::Value(v) => v,
    };
    if n == 0 { return Ok(StepResult::Value(String::new())); }
    if n <= options.len() { return Ok(StepResult::Value(options[n-1].clone())); }
    match ask_text(&format!("New {label}"), "", false)? {
        StepResult::Back => Ok(StepResult::Back),
        StepResult::Value(v) => {
            if n == options.len() + 2 { add_project_item(repo, key, &v)?; }
            Ok(StepResult::Value(v))
        }
    }
}

/// One terminal-based prompt flow with backwards navigation. This is invoked
/// from a Zed task rather than the VS Code Quick Pick API.
pub fn interactive(repo: &Path, config: &Config, lint: &Commitlint) -> Result<CommitMessage> {
    let mut msg = CommitMessage::default();
    let mut steps: Vec<&str> = vec!["type"];
    if config.prompt_scopes { steps.push("scope"); }
    if config.gitmoji { steps.push("gitmoji"); }
    if config.prompt_ci { steps.push("ci"); }
    if config.prompt_tag { steps.push("tag"); }
    steps.push("subject");
    if config.prompt_body { steps.push("body"); }
    if config.prompt_footer { steps.push("footer"); }
    let mut i = 0;
    while i < steps.len() {
        let result: StepResult<String> = match steps[i] {
            "type" => {
                let types = lint.types(&config.types);
                match choose("Commit type", &types, Some(&msg.kind), i > 0)? {
                    StepResult::Back => StepResult::Back,
                    StepResult::Value(n) => StepResult::Value(types[n].clone()),
                }
            }
            "scope" => pick_optional(repo, &lint.scopes(&config.scopes), "Scope", "scopes")?,
            "gitmoji" => {
                let mut list = vec!["(none)".to_string()];
                list.extend(EMOJIS.iter().map(|(code, emoji, meaning)| format!("{code} {emoji} — {meaning}")));
                list.push("(custom Gitmoji)".to_string());
                match choose("Gitmoji", &list, None, true)? {
                    StepResult::Back => StepResult::Back,
                    StepResult::Value(0) => StepResult::Value(String::new()),
                    StepResult::Value(n) if n == list.len()-1 => ask_text("Gitmoji code or emoji", "", false)?,
                    StepResult::Value(n) => StepResult::Value(if config.emoji_format == "emoji" { EMOJIS[n-1].1.to_string() } else { EMOJIS[n-1].0.to_string() }),
                }
            }
            "ci" => match choose("Skip CI?", &["No".to_string(), "Yes".to_string()], Some(if msg.ci { "Yes" } else { "No" }), true)? {
                StepResult::Back => StepResult::Back,
                StepResult::Value(n) => StepResult::Value(if n == 1 { "Yes" } else { "No" }.to_string()),
            },
            "tag" => pick_optional(repo, &config.tags, "Tag", "tags")?,
            "subject" => ask_text("Subject", &msg.subject, false)?,
            "body" => ask_text("Body (leave blank to skip)", &msg.body, true)?,
            "footer" => ask_text("Footer (leave blank to skip)", &msg.footer, true)?,
            _ => unreachable!(),
        };
        let value = match result {
            StepResult::Back => { i = i.saturating_sub(1); continue; }
            StepResult::Value(value) => value,
        };
        match steps[i] {
            "type" => msg.kind = value,
            "scope" => msg.scope = value,
            "gitmoji" => msg.gitmoji = value,
            "ci" => msg.ci = value == "Yes",
            "tag" => msg.tag = value,
            "subject" => msg.subject = value,
            "body" => msg.body = value,
            "footer" => msg.footer = value,
            _ => unreachable!(),
        }
        i += 1;
    }
    msg.normalize();
    println!("\n--- Commit preview ---\n{}\n----------------------", msg.serialize());
    if !Confirm::with_theme(&ColorfulTheme::default()).with_prompt("Use this commit message?")
        .default(true).interact()? {
        bail!("cancelled; no Git changes were made");
    }
    Ok(msg)
}
