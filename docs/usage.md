# Local Worker Usage Guide

Local Worker is a desktop app for turning a spare, logged-in Mac into an
always-ready development worker. You can create a GitHub Issue from anywhere,
add the configured label, and let the Mac pick up the work locally.

The app is intentionally local-first:

- It uses the local Git checkout.
- It shells out to your existing `gh`, `codex`, and `claude` CLI sessions.
- It stores app state in SQLite.
- It keeps task history in Markdown files inside each project.

## Requirements

Install and authenticate the tools you plan to use before running the app:

```bash
git --version
gh auth login
codex --version
claude --version
```

Only one agent backend is required. If you use Codex, install and authenticate
Codex CLI. If you use Claude, install and authenticate Claude Code CLI.

For development builds of this app, also install Node.js, pnpm, and Rust.

## First Run

Install dependencies:

```bash
pnpm install
```

Start the Tauri desktop app:

```bash
pnpm tauri:dev
```

The app window opens with a Worker Health strip at the top. Before adding work,
check that:

- `gh` is available.
- `codex` or `claude` is available.
- Polling is active.
- Autostart is enabled if you want the app to start when this Mac logs in.

The app only runs while the Mac is powered on, logged in, and connected to the
network.

## Worker Health

Worker Health is the fast status check for the spare Mac.

It shows:

- Polling state.
- Last sync and next sync.
- `gh`, `codex`, and `claude` availability.
- Autostart state.
- Whether a failure needs review.
- The primary action: `Complete setup`, `Sync now`, `Run next`, or
  `Review failure`.

If Worker Health says a CLI is missing, install or authenticate that CLI in the
same macOS user account that runs Local Worker.

## Create a New Project

Use `Create New Project` when you want Local Worker to scaffold a fresh
worker-ready repository.

The wizard has four steps:

1. `Name`: choose the project name and parent folder.
2. `GitHub`: choose whether to create a GitHub repository and whether it should
   be private or public.
3. `Automation`: choose `codex` or `claude`, set the test command, and expand
   advanced settings for the issue label and branch prefix.
4. `Review`: confirm the generated files and setup checks.

The default settings are:

- Issue label: `agent-task`
- Branch prefix: `codex`
- Test command: `bash scripts/smoke-test.sh`
- Backend: `codex`

When GitHub repository creation is enabled, Local Worker uses your existing
`gh` authentication. The app does not store GitHub tokens.

## Register an Existing Repository

Use `Register Existing Repo` when you already have a local Git checkout.

Before registering, confirm:

```bash
git status
git remote -v
gh repo view
```

The repository should have a GitHub `origin` remote that `gh` can access.

For best compatibility, the repository should include:

```text
AGENTS.md
tasks/queue.md
tasks/done.md
tasks/failed.md
```

If those files are missing, create them using the task format in
[../AGENTS.md](../AGENTS.md), or create a new project with the app and copy the
template files.

## GitHub Issue Workflow

Create an issue in the registered repository and add the configured label,
usually `agent-task`.

The issue should include:

- The desired change.
- Done criteria.
- Any constraints or files to avoid.
- The expected test command, if it differs from the project default.

Local Worker then:

1. Polls the repository on the configured schedule.
2. Finds open issues with the configured label.
3. Records the issue in SQLite to avoid duplicates.
4. Appends a `TODO-XXX` task to `tasks/queue.md`.
5. Starts the next open task if `Auto run` is enabled and the project is not busy.
6. Creates a branch using the configured branch prefix.
7. Runs Codex or Claude CLI in the project.
8. Runs the configured test command.
9. On success, updates `tasks/done.md`, commits, pushes, creates a pull request,
   and comments on the issue.
10. On failure, updates `tasks/failed.md`, comments on the issue, and leaves the
    worktree intact.

Local Worker does not merge pull requests. Review and merge PRs manually.

## Project Settings

Each project has polling and automation settings.

`Auto run` controls whether Local Worker should start work after syncing new
tasks. If it is off, issue sync still creates tasks, but you must press `Run`.

`Poll every N seconds` controls how often the app checks GitHub. Values below
30 seconds are clamped to 30 seconds.

The test command is the gate for success. A task is considered complete only
after the configured command exits successfully.

## Task Composer

Use `Compose Task` when you want to turn a local note into a queued task. The
composer opens as a right-side drawer so the dashboard stays available.

Task Composer is intentionally not an agent chat. Typing a request does not run
Codex or Claude, edit files, or start tests. It prepares a task draft with title,
priority, goal, done criteria, and constraints. The task is added to
`tasks/queue.md` only after you press `Create Task`.

If the selected project has `Auto run` enabled, Local Worker may start the next
open task after the new task is created. If a run is already active or the
worktree is not ready, the task stays queued and the app shows the reason.

When a failed or blocked run is selected, the Execution Log header shows
`Create follow-up`. That opens the same composer with failure context so you can
create a focused follow-up task.

## Failure Handling

Local Worker separates sync failures from run failures.

Common sync failures:

- `gh` is not installed.
- `gh` is not authenticated.
- The GitHub remote is missing or inaccessible.
- The network is unavailable.

Common run failures:

- The worktree has uncommitted non-task changes.
- `codex` or `claude` is unavailable.
- The agent exits with an error.
- The configured test command fails.

When a run fails, inspect:

```bash
git status
tasks/failed.md
tasks/queue.md
```

Then use the app's logs or `Create follow-up` to queue a focused repair task.
The worktree is left as-is so you can inspect or repair it manually.

## Safety Model

Local Worker is designed for a dedicated spare Mac or a dedicated user account.

The app does not:

- Run `git reset`.
- Discard local changes.
- Merge pull requests.
- Deploy automatically.
- Store GitHub tokens.
- Intentionally read `.env`, `.agent.env`, private keys, tokens, credentials,
  or secret files.

It refuses to start automated work when non-task files are already dirty. This
keeps user edits from being mixed with agent edits.

## Autostart

Use the `Start Local Worker when this Mac logs in` setting to enable the app's
recommended v1 background behavior.

This creates a LaunchAgent for the logged-in user. The app still requires the
Mac to be logged in. Running while logged out is not part of v1.

See [mac-launchd.md](mac-launchd.md) for advanced legacy notes about the earlier
Bash runner.

## Development Commands

Frontend build:

```bash
pnpm build
```

Rust tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Legacy task contract smoke test:

```bash
bash scripts/smoke-test.sh
```

Production Tauri bundle:

```bash
pnpm tauri:build
```

## Troubleshooting

Check GitHub authentication:

```bash
gh auth status
gh repo view
```

Check the selected repository state:

```bash
git status
git remote -v
```

Check CLI availability:

```bash
codex --version
claude --version
```

Run the configured test command manually:

```bash
bash scripts/smoke-test.sh
```

If polling is not updating, press `Sync` in the app and check Worker Health for
the last sync, next sync, and any sync error.

If an automated run will not start, check whether:

- `Auto run` is enabled.
- Another run is active.
- The worktree is dirty.
- There is an open task in `tasks/queue.md`.
- The selected backend is installed and authenticated.
