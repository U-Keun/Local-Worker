# Local Worker

Local Worker turns a spare home Mac into a GitHub-driven development worker.

The app watches registered repositories for GitHub Issues labeled `agent-task`. When it finds a new issue, it writes a compatible `tasks/queue.md` entry, runs Codex or Claude CLI against the repository, verifies the configured test command, commits the result, pushes a branch, opens a pull request, and comments back on the issue.

## What It Does

- Runs as a macOS-first Tauri desktop app with a menu bar tray.
- Uses your existing `gh`, `codex`, and `claude` CLI authentication.
- Stores project, run, log, and issue-sync state in SQLite.
- Keeps the original Markdown task files for review and CLI compatibility.
- Blocks runs when non-task files are already dirty.
- Never merges PRs automatically.

## Requirements

- macOS with the user logged in
- Git
- GitHub CLI authenticated with `gh auth login`
- Codex CLI and/or Claude Code CLI authenticated locally
- Node.js, pnpm, and Rust for development

## Development

Install dependencies:

```bash
pnpm install
```

Run the desktop app in development mode:

```bash
pnpm tauri:dev
```

Run frontend and Rust checks:

```bash
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

The legacy template smoke test still validates the repository contract:

```bash
bash scripts/smoke-test.sh
```

## Operating Model

1. Create a new project from the app or register an existing local Git repository.
2. Make sure the repository has a GitHub `origin` remote and `gh` can access it.
3. Create a GitHub Issue from anywhere and add the `agent-task` label.
4. Local Worker polls open labeled issues, creates a `TODO-XXX` entry in `tasks/queue.md`, and starts the first open task.
5. The worker creates a branch using the configured prefix, defaulting to `codex/`.
6. Codex or Claude makes the code changes.
7. Local Worker runs the project test command.
8. On success, Local Worker marks the task done, writes `tasks/done.md`, commits, pushes, creates a PR, and comments on the issue.
9. On failure, Local Worker marks the task blocked, writes `tasks/failed.md`, comments on the issue, and leaves the worktree intact.

## Creating Projects

The dashboard can create a fresh worker-ready repository:

1. Enter a project name and parent folder.
2. Choose whether to create a GitHub repository with `gh repo create`.
3. Local Worker creates the folder, runs `git init`, writes `AGENTS.md`, `tasks/`, `.gitignore`, `README.md`, and `scripts/smoke-test.sh`.
4. Local Worker creates the initial commit and registers the project in the app.

If GitHub creation is enabled, the app uses your existing `gh` authentication, adds `origin`, and pushes the initial commit.

## Safety Baseline

Local Worker is built for a dedicated spare machine or user account.

- It refuses to run when there are uncommitted non-task changes.
- It does not run `git reset`, discard work, merge PRs, or deploy.
- It does not store GitHub tokens. The app shells out to `gh`.
- It does not intentionally read `.env`, `.agent.env`, credentials, private keys, or token files.
- Failed runs leave the worktree as-is so a human can inspect the result.

## Project Compatibility Files

The app keeps these files in each registered repository:

```text
AGENTS.md
tasks/queue.md
tasks/done.md
tasks/failed.md
```

The existing Bash scripts remain as legacy compatibility tools while the Tauri app becomes the primary runner.

## Current Scope

v1 is macOS-first and requires the Mac to be powered on, connected, and logged in. The menu bar app can write a LaunchAgent plist so it starts when the user logs in. A launchd helper that runs while logged out is intentionally left for a later version.
