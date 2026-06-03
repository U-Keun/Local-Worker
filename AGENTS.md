# Agent Contract

This repository is prepared for coding agents. The agent must act like a careful junior developer working under review.

## Task format

Tasks live in `tasks/queue.md`.

A task should look like this:

```md
## TODO-001: Short task title
Status: open
Priority: medium

Goal:
Describe the intended result.

Done criteria:
- Observable condition 1
- Observable condition 2

Constraints:
- Do not change unrelated files.
- Keep the diff small.
```

Valid statuses:

- `open`
- `in-progress`
- `done`
- `blocked`

## Definition of done

A task is done only when:

- The done criteria are satisfied.
- The configured test command succeeds.
- The diff is small and related to the task.
- The completion is represented by one coherent commit.

## Development style

Prefer:

- small functions
- readable names
- simple control flow
- tests before broad refactors
- direct fixes over speculative abstractions

Avoid:

- large rewrites
- unrelated formatting churn
- hidden dependency changes
- editing generated files unless the task requires it
- changing public behavior without a test or note

## Reporting style

When a task succeeds, update `tasks/done.md` with:

- task ID
- summary
- test command
- commit hash
- follow-up notes

When a task fails, update `tasks/failed.md` with:

- task ID
- failure summary
- test command
- relevant error message
- suggested next step
