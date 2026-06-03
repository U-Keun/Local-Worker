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
: "${CLAUDE_MODEL:=}"
: "${CLAUDE_TIMEOUT_SECONDS:=600}"
: "${NOTIFY:=true}"

# Exit codes: 0=success, 2=task file missing, 3=dirty worktree, 4=no open tasks, 124=timeout, 127=claude not found
mkdir -p logs tasks
LOG_FILE="logs/$(date +%Y%m%d-%H%M%S)-agent-once.log"

notify() {
  [[ "$NOTIFY" == "true" ]] || return 0
  local title="$1" message="$2"
  if [[ "$(uname)" == "Darwin" ]]; then
    osascript -e "display notification \"${message}\" with title \"${title}\"" 2>/dev/null || true
  elif command -v notify-send >/dev/null 2>&1; then
    notify-send "$title" "$message" 2>/dev/null || true
  fi
}

on_exit() {
  local code=$?
  local project
  project="$(basename "$ROOT")"
  case "$code" in
    0)   notify "Dev Agent: done"    "$project — task completed" ;;
    4)   notify "Dev Agent: idle"    "$project — no open tasks" ;;
    124) notify "Dev Agent: timeout" "$project — timed out after ${CLAUDE_TIMEOUT_SECONDS}s" ;;
    *)   notify "Dev Agent: failed"  "$project — exit $code" ;;
  esac
}
trap on_exit EXIT

if ! command -v claude >/dev/null 2>&1; then
  echo "claude CLI was not found. Install Claude Code or adjust PATH." | tee "$LOG_FILE"
  exit 127
fi

if [[ ! -f "$TASK_FILE" ]]; then
  echo "Task file not found: $TASK_FILE" | tee "$LOG_FILE"
  exit 2
fi

if ! grep -q "^Status: open" "$TASK_FILE"; then
  echo "No open tasks in $TASK_FILE. Nothing to do." | tee -a "$LOG_FILE"
  exit 4
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
2. Update that task's status to in-progress in ${TASK_FILE}.
3. Create a brief implementation plan in your response.
4. Make the smallest coherent code or documentation changes needed.
5. Run this test command exactly: ${TEST_COMMAND}
6. If the test fails, inspect the failure and retry up to ${MAX_FIX_ATTEMPTS} times.
7. If successful and COMMIT_ON_SUCCESS is ${COMMIT_ON_SUCCESS}:
   - update the task status to done in ${TASK_FILE}
   - run git status
   - run git diff
   - git add only relevant files (including ${TASK_FILE})
   - commit with message: agent: complete <task-id>
8. If unsuccessful after all retries:
   - update the task status to blocked in ${TASK_FILE}
   - append a short report to tasks/failed.md
   - do not commit broken work.
9. In your final response, summarize:
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

if [[ -n "$CLAUDE_MODEL" ]]; then
  ARGS+=("--model" "$CLAUDE_MODEL")
fi

{
  echo "=== Local Dev Agent run ==="
  echo "Root: $ROOT"
  echo "Task file: $TASK_FILE"
  echo "Test command: $TEST_COMMAND"
  echo "Started: $(date)"
  echo
} | tee -a "$LOG_FILE"

TIMEOUT_ARGS=()
if [[ "$CLAUDE_TIMEOUT_SECONDS" != "0" ]]; then
  if command -v timeout >/dev/null 2>&1; then
    TIMEOUT_ARGS=("timeout" "$CLAUDE_TIMEOUT_SECONDS")
  elif command -v gtimeout >/dev/null 2>&1; then
    TIMEOUT_ARGS=("gtimeout" "$CLAUDE_TIMEOUT_SECONDS")
  else
    echo "Warning: timeout command not found. Install coreutils (macOS: brew install coreutils) to enable CLAUDE_TIMEOUT_SECONDS." | tee -a "$LOG_FILE"
  fi
fi

set +e
"${TIMEOUT_ARGS[@]}" claude "${ARGS[@]}" -p "$PROMPT" 2>&1 | tee -a "$LOG_FILE"
STATUS=${PIPESTATUS[0]}
set -e

if [[ "$STATUS" -eq 124 ]]; then
  echo "Claude timed out after ${CLAUDE_TIMEOUT_SECONDS} seconds." | tee -a "$LOG_FILE"
fi

echo "Claude exit status: $STATUS" | tee -a "$LOG_FILE"
exit "$STATUS"
