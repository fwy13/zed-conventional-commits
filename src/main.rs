mod config;
mod git;
mod lint;
mod message;
mod prompt;
mod zed;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use config::Config;
use lint::Commitlint;
use message::CommitMessage;
use std::{env, fs, io::IsTerminal, path::{Path, PathBuf}};

#[derive(Debug, Parser)]
#[command(name = "zed-cc", version, about = "Conventional Commits for Zed — native Rust CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Interactively compose a message and commit staged Git changes (default)
    Commit(CommitArgs),
    /// Produce a commit message without modifying the Git repository
    Compose(MessageArgs),
    /// Check a complete conventional commit message
    Check(CheckArgs),
    /// Merge the two Conventional Commit tasks into Zed tasks.json
    InitZed {
        /// Install into ~/.config/zed/tasks.json instead of .zed/tasks.json
        #[arg(long)]
        global: bool,
        /// Repository directory for project-local tasks
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Absolute path to zed-cc to use in generated Zed tasks (avoids GUI PATH issues)
        #[arg(long)]
        bin: Option<PathBuf>,
    },
    /// Save a selectable scope in .zed-conventional-commits.toml
    AddScope { scope: String, #[arg(long)] repo: Option<PathBuf> },
    /// Save a selectable tag in .zed-conventional-commits.toml
    AddTag { tag: String, #[arg(long)] repo: Option<PathBuf> },
}

#[derive(Debug, Clone, Args, Default)]
struct MessageArgs {
    /// Git working directory or repo root
    #[arg(long)]
    repo: Option<PathBuf>,
    /// Commit type, e.g. feat, fix, docs
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(long)]
    scope: Option<String>,
    #[arg(long)]
    gitmoji: Option<String>,
    #[arg(long)]
    subject: Option<String>,
    #[arg(long)]
    body: Option<String>,
    #[arg(long)]
    footer: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    /// Append [skip ci] to header
    #[arg(long)]
    skip_ci: bool,
}

#[derive(Debug, Clone, Args, Default)]
struct CommitArgs {
    #[command(flatten)]
    message: MessageArgs,
    /// Preview only; do not stage or commit
    #[arg(long)]
    dry_run: bool,
    /// Output a message, do not commit
    #[arg(long)]
    no_commit: bool,
    /// Explicitly stage every change (including untracked files) before committing
    #[arg(long)]
    all: bool,
    /// Push after successful commit (never implied by default)
    #[arg(long)]
    push: bool,
    /// Open git's editor for final message review
    #[arg(long)]
    edit: bool,
    /// Save the generated message to the specified file
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[arg(long)]
    repo: Option<PathBuf>,
    /// Literal commit message (alternative to --file)
    #[arg(long, conflicts_with = "file")]
    message: Option<String>,
    #[arg(long)]
    file: Option<PathBuf>,
}

fn cwd_or(path: Option<PathBuf>) -> Result<PathBuf> {
    Ok(match path { Some(path) => path, None => env::current_dir()? })
}

fn repo_for_config(path: &Path) -> PathBuf {
    git::root(path).unwrap_or_else(|_| path.to_path_buf())
}

fn message_from_args(args: MessageArgs, cfg: &Config, lint: &Commitlint, repo: &Path) -> Result<CommitMessage> {
    let MessageArgs { kind, scope, gitmoji, subject, body, footer, tag, skip_ci, .. } = args;
    let mut msg = if kind.is_none() && subject.is_none() {
        if !std::io::stdin().is_terminal() {
            bail!("interactive mode needs a terminal; pass --type and --subject for non-interactive use");
        }
        prompt::interactive(repo, cfg, lint)?
    } else {
        let (Some(kind), Some(subject)) = (kind, subject) else {
            bail!("non-interactive mode requires both --type and --subject");
        };
        CommitMessage {
            kind, scope: scope.unwrap_or_default(), gitmoji: gitmoji.unwrap_or_default(),
            subject, body: body.unwrap_or_default(), footer: footer.unwrap_or_default(),
            tag: tag.unwrap_or_default(), ci: skip_ci,
        }
    };
    if !cfg.line_break.is_empty() {
        for part in [&mut msg.subject, &mut msg.body, &mut msg.footer] {
            *part = part.replace(&cfg.line_break, "\n");
        }
    }
    msg.normalize();
    lint.validate(&msg)?;
    Ok(msg)
}

fn compose(args: MessageArgs) -> Result<()> {
    let path = cwd_or(args.repo.clone())?;
    let repo = repo_for_config(&path);
    let cfg = Config::load(&repo)?;
    let lint = Commitlint::load(&repo)?;
    let message = message_from_args(args, &cfg, &lint, &repo)?;
    println!("{}", message.serialize());
    Ok(())
}

fn commit(args: CommitArgs) -> Result<()> {
    let path = cwd_or(args.message.repo.clone())?;
    let repo = git::root(&path)?;
    let cfg = Config::load(&repo)?;
    let lint = Commitlint::load(&repo)?;
    // Compose before touching index. Invalid configs, cancelled prompts and lint
    // failures never stage or commit anything.
    let msg = message_from_args(args.message, &cfg, &lint, &repo)?;
    let serialized = msg.serialize();
    let preview = args.dry_run || args.no_commit || !cfg.auto_commit;
    if let Some(path) = &args.output {
        fs::write(path, format!("{serialized}\n"))
            .with_context(|| format!("writing {}", path.display()))?;
        println!("Message written to {}", path.display());
    }
    if preview {
        println!("{serialized}");
        println!("\n[Preview only — no git add, commit or push]");
        return Ok(());
    }
    if args.all || cfg.stage_all { git::stage_all(&repo)?; }
    if !git::has_staged_changes(&repo)? {
        bail!("no staged changes; stage files in Zed's Git panel or rerun with --all to stage everything");
    }
    git::commit(&repo, &serialized, args.edit)?;
    if args.push || cfg.auto_push {
        git::push(&repo)?;
    }
    println!("Conventional commit created successfully");
    Ok(())
}

fn check(args: CheckArgs) -> Result<()> {
    let path = cwd_or(args.repo)?;
    let repo = repo_for_config(&path);
    let lint = Commitlint::load(&repo)?;
    let text = match (args.message, args.file) {
        (Some(message), None) => message,
        (None, Some(file)) => fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?,
        _ => bail!("pass either --message or --file"),
    };
    let (header, body) = text.split_once("\n\n").unwrap_or((&text, ""));
    let (left, subject) = header.split_once(": ").context("header must contain ': ' separator")?;
    let (kind, scope) = if let Some((t, sc)) = left.strip_suffix(')').and_then(|x| x.split_once('(')) {
        (t.to_string(), sc.to_string())
    } else { (left.to_string(), String::new()) };
    let mut msg = CommitMessage { kind, scope, subject: subject.into(), body: body.into(), ..Default::default() };
    msg.normalize();
    lint.validate(&msg)?;
    println!("Valid conventional commit message");
    Ok(())
}

fn run() -> Result<()> {
    match Cli::parse().command.unwrap_or(Command::Commit(CommitArgs::default())) {
        Command::Commit(args) => commit(args),
        Command::Compose(args) => compose(args),
        Command::Check(args) => check(args),
        Command::InitZed { global, repo, bin } => {
            let repo = cwd_or(repo)?;
            let file = zed::install_tasks_with_bin(&repo, global, bin.as_deref())?;
            println!("Zed tasks installed: {}\nUse Command Palette > task: spawn", file.display());
            Ok(())
        }
        Command::AddScope { scope, repo } => {
            println!("Saved scope in {}", config::add_project_item(&repo_for_config(&cwd_or(repo)?), "scopes", &scope)?.display());
            Ok(())
        }
        Command::AddTag { tag, repo } => {
            println!("Saved tag in {}", config::add_project_item(&repo_for_config(&cwd_or(repo)?), "tags", &tag)?.display());
            Ok(())
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}
