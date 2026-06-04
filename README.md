# Local Worker

Local Worker turns a spare home Mac into a GitHub-driven development worker.

Create or register a local repository, label a GitHub Issue with `agent-task`,
and the Local Worker desktop app will sync the issue, create a Markdown task,
run Codex or Claude CLI, verify the configured test command, push a branch,
open a pull request, and report back to the original issue.

## Current App

- macOS-first Tauri desktop app with a menu bar tray.
- Project creation wizard for worker-ready repositories.
- GitHub Issue polling per project, with configurable interval and `Auto run`.
- Worker Health dashboard for polling, CLI availability, autostart, and sync state.
- SQLite state for projects, issue sync, runs, logs, pull requests, and Agent Chat.
- Agent Chat drawer for project questions or failed-run intervention.
- Markdown task files kept for review and CLI template compatibility.
- Safety rules that avoid automatic reset, discard, merge, or token storage.

## Requirements

- macOS with the user logged in.
- Git.
- GitHub CLI authenticated with `gh auth login`.
- Codex CLI and/or Claude Code CLI installed and authenticated locally.
- Node.js, pnpm, and Rust for development builds.

## Quick Start

Install dependencies:

```bash
pnpm install
```

Run the desktop app in development mode:

```bash
pnpm tauri:dev
```

Open the app, confirm Worker Health, then create a new project or register an
existing repository. For the full operating guide, read
[docs/usage.md](docs/usage.md).

## Operating Model

1. A project is registered in the app.
2. The app polls the project's GitHub repository for open issues labeled
   `agent-task`.
3. New issues become `TODO-XXX` entries in `tasks/queue.md`.
4. If `Auto run` is enabled, the first open task is assigned to Codex or Claude.
5. The app runs the configured test command.
6. Passing runs are committed, pushed, turned into pull requests, and reported on
   the issue.
7. Failed runs are marked blocked, reported in `tasks/failed.md`, and left for
   human inspection or Agent Chat intervention.

## Project Files

Local Worker keeps the original task contract inside each worker-managed
repository:

```text
AGENTS.md
tasks/queue.md
tasks/done.md
tasks/failed.md
```

These files make the desktop app compatible with the earlier CLI template and
keep the work auditable from Git.

## Development Commands

```bash
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
bash scripts/smoke-test.sh
pnpm tauri:build
```

## Documentation

- [Detailed usage guide](docs/usage.md)
- [Advanced legacy launchd notes](docs/mac-launchd.md)
- [Agent task contract](AGENTS.md)

## Scope

v1 is designed for a powered-on Mac that is logged in and connected to the
network. The recommended background mode is the Tauri app plus its autostart
toggle. A helper that runs while the Mac is logged out is intentionally outside
the v1 scope.
