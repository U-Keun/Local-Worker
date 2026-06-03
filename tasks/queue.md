# Task Queue

The agent should pick the first task with `Status: open`.

## TODO-001: Verify the template smoke test
Status: open
Priority: high

Goal:
Run the provided smoke test and make sure the template is internally consistent.

Done criteria:
- `bash scripts/smoke-test.sh` succeeds.
- The agent records the result in `tasks/done.md`.

Constraints:
- Do not add external dependencies.
- Keep the change small.
- Do not modify secrets or local config files.
