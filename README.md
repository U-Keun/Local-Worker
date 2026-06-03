# Local Dev Agent Template

Turn an unused Mac or Linux machine into a local development worker for coding agents.

This template is intentionally small. It gives you a repeatable loop:

```text
tasks/queue.md
  -> agent-once.sh
  -> Claude Code headless run
  -> code/docs changes
  -> test command
  -> commit or failure report
```

## Goals

- Run one coherent development task per agent invocation.
- Keep every run reviewable through logs, git diff, and commits.
- Avoid direct changes to `main` or `master`.
- Prevent obvious unsafe operations such as reading secrets, pushing, using sudo, or deleting unrelated files.
- Stay agent-agnostic enough to later support Codex, Aider, OpenHands, Gemini CLI, or a custom runner.

## Requirements

- macOS or Linux
- Git
- Claude Code CLI available as `claude`
- A project-level test command
- Optional: GitHub CLI `gh` for creating and pushing the repository

## Quick start

From this template repository:

```bash
chmod +x scripts/*.sh
./scripts/setup.sh
```

`setup.sh` starts an interactive Claude session that asks a few questions and writes `.agent.env` for you.

For the template itself, the default test command is:

```bash
bash scripts/smoke-test.sh
```

Run one development cycle:

```bash
./scripts/agent-once.sh
```

Run repeatedly:

```bash
./scripts/agent-loop.sh
```

The loop is deliberately conservative. If a run leaves uncommitted changes, the next run stops until you review the result.

## Recommended GitHub setup

```bash
mkdir local-dev-agent-template
cd local-dev-agent-template
# copy these template files here

git init
git add .
git commit -m "init local dev agent template"
gh repo create local-dev-agent-template --public --source=. --push
```

Then enable the repository as a template from GitHub repository settings.

## How to use this inside another project

Copy the following files/folders into the root of the target project:

```text
CLAUDE.md
AGENTS.md
.agent.env.example
tasks/
scripts/
logs/.gitkeep
.claude/settings.example.json
```

Then:

```bash
cp .agent.env.example .agent.env
$EDITOR .agent.env
```

Set the real test command, for example:

```bash
TEST_COMMAND="npm test"
# or
TEST_COMMAND="cargo test"
# or
TEST_COMMAND="cmake --build build && ctest --test-dir build"
```

Add a task to `tasks/queue.md`, then run:

```bash
./scripts/agent-once.sh
```

## File structure

```text
.
├── README.md
├── CLAUDE.md
├── AGENTS.md
├── SECURITY.md
├── .agent.env.example
├── .gitignore
├── .claude/
│   └── settings.example.json
├── docs/
│   └── mac-launchd.md
├── logs/
│   └── .gitkeep
├── scripts/
│   ├── agent-once.sh
│   ├── agent-loop.sh
│   ├── add-task.sh
│   ├── watch-issues.sh
│   ├── setup.sh
│   └── smoke-test.sh
└── tasks/
    ├── queue.md
    ├── done.md
    └── failed.md
```

## Adding tasks

**Manually:**

```bash
bash scripts/add-task.sh --title "Fix login bug" --goal "에러 메시지가 표시되지 않는 문제 수정"
```

**From GitHub Issues (polling):**

```bash
bash scripts/watch-issues.sh &
```

`agent-task` 라벨이 붙은 open issue를 주기적으로 확인해서 `queue.md`에 자동으로 추가합니다. 기본 폴링 간격은 60초이며 `.agent.env`에서 `WATCH_INTERVAL_SECONDS`로 조정할 수 있습니다.

## GitHub Actions

Two optional workflows are included in `.github/workflows/`.

> **Note:** When creating a new repository from this template, workflow files are copied automatically but Secrets and Variables are not. You must configure these manually in each new repository's settings.

### Issue → Task sync

When you label a GitHub Issue with `agent-task`, the workflow automatically appends a task entry to `tasks/queue.md` and commits it to the default branch. A comment is posted on the issue with the assigned task ID.

One-time setup per project:

1. Create the `agent-task` label in your repository (Settings → Labels).

No secrets or tokens needed — the workflow uses the built-in `GITHUB_TOKEN`.

### Agent branch CI

Runs the configured test command whenever a commit is pushed to an `agent/**` branch.

By default it runs `bash scripts/smoke-test.sh`. To use a project-specific test command, set a repository variable:

- Settings → Secrets and variables → Actions → Variables → New repository variable
- Name: `TEST_COMMAND`
- Value: e.g. `npm test` or `cargo test`

For projects that need a build environment (Node, Rust, Python, etc.), add the relevant setup steps to `.github/workflows/agent-ci.yml` — the file includes commented examples.

## Operating model

Each run should do exactly one task:

1. Read `CLAUDE.md`, `AGENTS.md`, and `tasks/queue.md`.
2. Pick the first open task.
3. Make the smallest coherent change.
4. Run the configured test command.
5. Retry fixes a limited number of times.
6. Commit only if the task is verified.
7. Write a failure report if verification fails.

## Permission model

This template does not enable unrestricted execution by default.

Use Claude Code permissions intentionally. You can start by copying `.claude/settings.example.json` to `.claude/settings.local.json` and adjusting it for your project:

```bash
cp .claude/settings.example.json .claude/settings.local.json
```

Keep local settings out of git. The provided `.gitignore` already ignores `.claude/settings.local.json`.

## First real task suggestion

After pushing the template repository, use it on a small personal project and add a task like this:

```md
## TODO-001: Add a smoke test
Status: open
Priority: high

Goal:
Add a minimal smoke test that verifies the project starts or builds.

Done criteria:
- The test command succeeds locally.
- README explains how to run the test.

Constraints:
- Keep the change small.
- Do not refactor unrelated code.
```
