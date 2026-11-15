# Conventional Commits for Zed — Rust port

A native Rust rewrite of the **commit-message workflow** from
[vivaxy/vscode-conventional-commits](https://github.com/vivaxy/vscode-conventional-commits).
It works in Zed through **Tasks / integrated terminal** and is also a
standalone CLI. No VS Code runtime is needed.

> Important: this is **not** a `.wasm` Zed extension. Zed's public extension API
> does not offer VS Code's `scm/title`, `registerCommand`, `QuickPick`, or
> `Repository.inputBox` interfaces. Claims of an identical native Git toolbar
> extension would be inaccurate. Instead the Zed task launches the Rust CLI.
> See [Zed Tasks](https://zed.dev/docs/tasks) and
> [Zed extension docs](https://zed.dev/docs/extensions/developing-extensions).

## Features / Migration coverage

| Original VS Code feature | This Rust port |
|---|---|
| type + scope + subject | Yes, interactive terminal or CLI arguments |
| Gitmoji code / emoji | Yes, curated built-in list + arbitrary custom input; upstream JSON not in supplied Repomix |
| tags / `[skip ci]` | Yes |
| body / footer / multiline message | Yes |
| backward navigation through prompts | Yes (`← Back`, `:back` for text) |
| project scopes and tags | Yes, TOML config and `add-scope`/`add-tag` |
| commitlint JSON/YAML rules | Common upstream-supported rules natively |
| JS/TS / extends / plugins / full commitlint | Yes **if** project provides `@commitlint/cli`; CLI resolves/lints rules |
| automatic commit | Yes, **staged changes only** unless `--all` configured |
| push | Optional `--push` / `auto_push=true` |
| message preview | `--dry-run`, `--no-commit` or `compose` |
| edit message in Git editor | `--edit` |
| SCM toolbar button / QuickPick / input box | Not exposed in current Zed extension API |
| version pop-up/reset GlobalState | Not applicable to CLI |

By default, **push is OFF** and **stage-all is OFF**. The original extension's
`git.postCommitCommand=sync` belongs to VS Code, so this port does not guess
that Git push is desired. Only staged files will be committed, unless you
explicitly pass `--all` (which includes untracked files).

## Requirements

- Zed editor (optional, for editor integration)
- Rust stable and Cargo (install at <https://rustup.rs/>)
- Git installed and configured with `user.name` / `user.email`
- For JS/TS Commitlint configs only: a Node.js project with locally installed
  `@commitlint/cli` (`npm install -D @commitlint/cli`)

## Install and build (Linux Mint / Ubuntu / macOS / Windows)

```bash
# unzip this project and cd into its root
rustup update stable
cargo build --release
cargo test --all-targets
cargo install --path .
zed-cc --help
```

`cargo install --path .` places `zed-cc` in `~/.cargo/bin`. Make sure that
`~/.cargo/bin` is on PATH (restart Zed after changing PATH). On Linux Mint,
`sudo apt install git build-essential pkg-config` installs typical prerequisites;
Rust itself should come from rustup.

**Build artifact:** `target/release/zed-cc` (Linux/macOS), `target/release/zed-cc.exe`
(Windows). This is a native executable, not a `.vsix`.

## Zed setup

**Global** (every Git project):

```bash
zed-cc init-zed --global
```

**Only the current project:**

```bash
cd /path/to/your/git-repo
zed-cc init-zed
```

This merges two tasks into `~/.config/zed/tasks.json` (global) or
`.zed/tasks.json` (project). Existing JSON tasks are preserved and backed up to
`tasks.json.bak`. If your tasks file contains JSONC comments or trailing
commas, the installer refuses to overwrite it. In that case, manually merge
`examples/tasks.json` with your task file.

Open Zed → **Command Palette** (`Ctrl+Shift+P` on Linux) →
`task: spawn` → **Conventional Commit (Rust)** (or **Preview**).
Zed opens its integrated terminal and asks the questions.

Optionally merge `examples/keymap.json` into `~/.config/zed/keymap.json` to
bind `Ctrl+Alt+C` to the commit task; do **not** replace existing keybindings.
Task definitions use `$ZED_WORKTREE_ROOT` as documented by Zed.

## Typical workflow

1. Open a Git repository in Zed, make changes, and **stage** desired files
   in the Git panel.
2. Run task `Conventional Commit (Rust)`.
3. Select type, scope, Gitmoji, optional CI/tag, then enter subject/body/footer.
4. Review the message and confirm. The CLI runs `git commit -F -` with the
   validated message. `--push` is opt-in.

A preview-only task is provided; it does not change Git state.

## CLI use cases

```bash
# Interactive, commit only staged changes
zed-cc commit

# Preview without any Git writes
zed-cc commit --dry-run

# Form a message without a Git repo
zed-cc compose --type feat --scope api --gitmoji :sparkles: \
  --subject 'add registration endpoint' --body 'Implement validation.' \
  --footer 'Refs: #14'

# Commit staged files without any prompts
zed-cc commit --type fix --scope auth --subject 'handle expired token'

# Deliberately stage all changed/untracked files
zed-cc commit --all --type docs --subject 'update documentation'

# Push to configured upstream, after successfully committing
zed-cc commit --push --type chore --subject 'update dependencies'

# Review the message with Git's configured editor
zed-cc commit --edit --type feat --subject 'add dashboard'

# Save message to disk without committing
zed-cc commit --no-commit --output /tmp/COMMIT_EDITMSG \
  --type feat --subject 'add dashboard'

# Lint a conventional commit header
zed-cc check --message 'feat(api): add dashboard'

# Save project scope/tag
zed-cc add-scope backend
zed-cc add-tag '[release]'
```

Interactive mode requires a terminal. For CI or scripts, provide both `--type`
and `--subject`; no prompt will appear. `--repo` selects a repository even when
invoked from another working directory.

## Config files

Copy `examples/config.toml` to your Git root and rename it to
`.zed-conventional-commits.toml`. This overrides global settings. Alternatively
put a config at `~/.config/zed-conventional-commits/config.toml`. Settings not
specified in a project config inherit the global/default value.

```toml
# .zed-conventional-commits.toml
auto_commit = true
auto_push = false
stage_all = false
gitmoji = true
emoji_format = 'code'
scopes = ['api', 'web']
tags = ['[release]']
prompt_ci = true
prompt_tag = true
```

`line_break = "\\n"` lets you enter literal `\n` in body/footer to represent a
newline; leave empty for no replacement.

### Commitlint compatibility

- Native loading: `.commitlintrc`, `.commitlintrc.json`,
  `.commitlintrc.yaml`, `.commitlintrc.yml`, `commitlint.config.json`, or the
  `commitlint` section of `package.json`.
- Native checks cover `type`, `scope`, `subject`, `header`, `body`, `footer`
  rules for `enum`, `empty`, `case`, `full-stop`, `min-length`, `max-length`.
  Complex case-conversion semantics may differ from the official JS engine.
- With installed `@commitlint/cli`, it uses `--print-config json` and runs
  the **real Commitlint CLI** against the completed message. This supports
  configuration presets/extends, plugins and TS/JS configs just like the
  original repository's Commitlint integration. No packages are fetched
  automatically. A broken or unavailable dynamic config fails explicitly.
- For full fidelity to official Commitlint rules, install `@commitlint/cli`
  in your project even when your configuration is JSON/YAML.

Example:

```bash
npm install --save-dev @commitlint/cli @commitlint/config-conventional
# commitlint.config.mjs:
# export default { extends: ['@commitlint/config-conventional'] };
zed-cc commit --dry-run --type feat --subject 'check plugin config'
```

## Tests

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release
./scripts/smoke.sh
```

Coverage includes unit tests for original message serialization, TOML defaults,
JSON/YAML commitlint config, task merging, and Git integration tests for staged
files, no-staged error, safe dry run, project rules and paths containing spaces.
GitHub Actions CI is provided for Linux, macOS and Windows.

## Limitations and status

- The source provided in Repomix contains only text files and **does not
  include** `src/vendors/gitmojis.json`, image assets, or lockfiles. The emoji
  picker consequently uses a curated built-in set rather than claiming that
  every original Gitmoji entry was migrated.
- This is an original Rust **port**, not a drop-in VS Code extension.
- The Rust code comes with tests, CI and build instructions, but executables
  are **not included in this archive**; compile with Cargo for your OS.
- A test suite does not imply it passed until `cargo test` is executed in an
  environment with Rust and dependencies installed.

## Attribution / license

Based on `vivaxy/vscode-conventional-commits`, MIT licensed, © 2020 vivaxy.
See `LICENSE`. This project preserves the MIT notice and references the
source while adapting the implementation to Rust and Zed Tasks.

## Distributing to other users

See [DISTRIBUTION.vi.md](DISTRIBUTION.vi.md). The release workflow builds Linux x86_64/aarch64, macOS x86_64/aarch64, and Windows x86_64 binaries and publishes SHA256 checksums for GitHub Releases. The install scripts accept `OWNER/REPO`, install an appropriate prebuilt binary, and register the global Zed tasks with an absolute path. For source installation: `cargo install --git https://github.com/OWNER/REPO.git && zed-cc init-zed --global`. There is no marketplace extension/VSIX packaging, since the required custom Git UI extension API does not currently exist in Zed.
