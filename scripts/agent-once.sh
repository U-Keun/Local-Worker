#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

if [[ -f ".agent.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source ".agent.env"
  set +a
fi

: "${TASK_FILE:=tasks/queue.md}"
: "${TEST_COMMAND:=bash scripts/smoke-test.sh}"
: "${MAX_FIX_ATTEMPTS:=3}"
: "${CREATE_BRANCH:=true}"
: "${BRANCH_PREFIX:=agent}"
: "${BASE_BRANCHES:=main master}"
: "${REQUIRE_CLEAN_WORKTREE:=true}"
: "${COMMIT_ON_SUCCESS:=true}"
: "${CLAUDE_OUTPUT_FORMAT:=text}"
: "${CLAUDE_PERMISSION_MODE:=}"
: "${CLAUDE_BARE:=false}"
: "${CLAUDE_SETTINGS_FILE:=}"

mkdir -p logs tasks
LOG_FILE="logs/$(date +%Y%m%d-%H%M%S)-agent-once.log"

if ! command -v claude >/dev/null 2>&1; then
  echo "claude CLI was not found. Install Claude Code or adjust PATH." | tee "$LOG_FILE"
  exit 127
fi

if [[ ! -f "$TASK_FILE" ]]; then
  echo "Task file not found: $TASK_FILE" | tee "$LOG_FILE"
  exit 2
fi

if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  CURRENT_BRANCH="$(git branch --show-current || true)"

  if [[ "$REQUIRE_CLEAN_WORKTREE" == "true" ]]; then
    if ! git diff --quiet || ! git diff --cached --quiet; then
      {
        echo "Worktree is not clean. Review, commit, or stash changes first."
        git status --short
      } | tee "$LOG_FILE"
      exit 3
    fi
  fi

  if [[ "$CREATE_BRANCH" == "true" ]]; then
    for base in $BASE_BRANCHES; do
      if [[ "$CURRENT_BRANCH" == "$base" ]]; then
        NEW_BRANCH="$BRANCH_PREFIX/$(date +%Y%m%d-%H%M%S)"
        git checkout -b "$NEW_BRANCH" | tee -a "$LOG_FILE"
        break
      fi
    done
  fi
fi

read -r -d '' PROMPT <<PROMPT || true
You are running as a local development worker.

Read these files before making changes:
- CLAUDE.md
- AGENTS.md
- ${TASK_FILE}

Run exactly one work cycle:
1. Pick the first task with Status: open from ${TASK_FILE}.
2. Create a brief implementation plan in your response.
3. Make the smallest coherent code or documentation changes needed.
4. Run this test command exactly: ${TEST_COMMAND}
5. If the test fails, inspect the failure and retry up to ${MAX_FIX_ATTEMPTS} times.
6. If successful and COMMIT_ON_SUCCESS is ${COMMIT_ON_SUCCESS}:
   - run git status
   - run git diff
   - git add only relevant files
   - commit with message: agent: complete <task-id>
7. If unsuccessful:
   - append a short report to tasks/failed.md
   - do not commit broken work.
8. In your final response, summarize:
   - selected task
   - files changed
   - test command and result
   - commit hash if committed
   - risks or follow-up tasks

Hard safety rules:
- Never read or modify .env, .agent.env, credentials, SSH keys, tokens, or private config.
- Never push to remote.
- Never run sudo.
- Never delete unrelated files.
- Never modify main/master directly.
- Stop after one coherent task.
PROMPT

ARGS=()

if [[ "$CLAUDE_BARE" == "true" ]]; then
  ARGS+=("--bare")
fi

if [[ -n "$CLAUDE_PERMISSION_MODE" ]]; then
  ARGS+=("--permission-mode" "$CLAUDE_PERMISSION_MODE")
fi

if [[ -n "$CLAUDE_OUTPUT_FORMAT" ]]; then
  ARGS+=("--output-format" "$CLAUDE_OUTPUT_FORMAT")
fi

if [[ -n "$CLAUDE_SETTINGS_FILE" ]]; then
  ARGS+=("--settings" "$CLAUDE_SETTINGS_FILE")
fi

{
  echo "=== Local Dev Agent run ==="
  echo "Root: $ROOT"
  echo "Task file: $TASK_FILE"
  echo "Test command: $TEST_COMMAND"
  echo "Started: $(date)"
  echo
} | tee -a "$LOG_FILE"

set +e
claude "${ARGS[@]}" -p "$PROMPT" 2>&1 | tee -a "$LOG_FILE"
STATUS=${PIPESTATUS[0]}
set -e

echo "Claude exit status: $STATUS" | tee -a "$LOG_FILE"
exit "$STATUS"
