# Claude Development Rules

You are a local development worker operating inside this repository.

Your job is not to maximize the amount of code changed. Your job is to finish one small task safely, verify it, and leave a clear record.

## Local Worker app workflow

When invoked by the Local Worker desktop app, the app is the runner. The app
selects the task, updates task status, runs the configured verification command,
commits, pushes, creates the pull request, and comments on the GitHub Issue.

For each app run:

1. Read `AGENTS.md`.
2. Read `tasks/queue.md`.
3. Work only on the task named in the prompt.
4. Make the smallest coherent change that satisfies the done criteria.
5. Do not commit, push, create a pull request, merge, deploy, or modify secrets.
6. Stop after one coherent task and summarize changed files, risks, and any tests you ran manually.

## Legacy script workflow

For each run:

1. Read `AGENTS.md`.
2. Read `tasks/queue.md`.
3. Select the first task whose status is `open`.
4. Update that task's status to `in-progress` in `tasks/queue.md`.
5. Work only on that task.
6. Make the smallest coherent change that satisfies the done criteria.
7. Run the exact test command provided by the runner.
8. If the test fails, inspect the failure and retry within the allowed retry limit.
9. If verification succeeds: update the task status to `done` in `tasks/queue.md`, then commit.
10. If verification fails after all retries: update the task status to `blocked` in `tasks/queue.md`.
11. Stop after one coherent task.

## Hard safety rules

Never read, print, edit, copy, move, or summarize these files or directories:

- `.env`
- `.env.*`
- `.agent.env`
- private keys
- SSH keys
- API tokens
- credentials
- password files
- `~/.ssh/`
- `~/.aws/`
- `~/.config/gh/`
- unrelated personal files outside this repository

Never run:

- `sudo`
- `rm -rf` on broad paths
- destructive cleanup outside this repository
- `git push`
- deployment commands
- package publishing commands
- commands that modify global system configuration

Never modify `main` or `master` directly. If you detect that you are on `main` or `master`, stop and report the issue unless the runner already created a branch for you.

## Git rules

Before committing:

```bash
git status
git diff
```

Commit only relevant files.

Use this commit format:

```text
agent: complete TODO-XXX
```

If there is no clear task ID, use:

```text
agent: complete one queued task
```

Never push. A human reviews and pushes.

## Failure handling

If you cannot complete the task safely:

1. Do not commit.
2. Update the task status to `blocked` in `tasks/queue.md`.
3. Append a short note to `tasks/failed.md`.
4. Include the task ID, what you tried, the exact test command, and the failure reason.
5. Stop.

## Completion note

At the end of the run, report:

- selected task
- files changed
- test command
- test result
- commit hash, if committed
- remaining risks or follow-up tasks
