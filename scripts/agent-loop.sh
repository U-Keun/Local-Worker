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

: "${LOOP_INTERVAL_SECONDS:=1800}"
: "${MAX_RUNS:=0}"

RUN_COUNT=0

while true; do
  RUN_COUNT=$((RUN_COUNT + 1))
  echo "=== agent loop run ${RUN_COUNT} at $(date) ==="

  set +e
  ./scripts/agent-once.sh
  STATUS=$?
  set -e

  echo "agent-once exit status: ${STATUS}"

  if [[ "$STATUS" -eq 4 ]]; then
    echo "No open tasks remaining. Exiting loop."

    CURRENT_BRANCH="$(git branch --show-current || true)"
    BASE_BRANCHES="${BASE_BRANCHES:-main master}"
    ON_BASE=false
    for base in $BASE_BRANCHES; do
      if [[ "$CURRENT_BRANCH" == "$base" ]]; then
        ON_BASE=true
        break
      fi
    done

    if [[ "$ON_BASE" == "false" ]] && command -v gh >/dev/null 2>&1; then
      BASE_BRANCH="main"
      for base in $BASE_BRANCHES; do
        if git show-ref --verify --quiet "refs/remotes/origin/$base"; then
          BASE_BRANCH="$base"
          break
        fi
      done

      AHEAD=$(git rev-list --count "origin/${BASE_BRANCH}..HEAD" 2>/dev/null || echo 0)
      if [[ "$AHEAD" -gt 0 ]]; then
        DONE_TASKS=$(grep -A1 "^## TODO-" tasks/queue.md | grep "Status: done" | wc -l | tr -d ' ')
        echo "Creating PR for branch ${CURRENT_BRANCH} (${DONE_TASKS} task(s) done)."
        gh pr create \
          --title "agent: completed ${DONE_TASKS} task(s) on ${CURRENT_BRANCH}" \
          --body "$(cat <<BODY
## Summary

Automated agent run completed all queued tasks.

- Branch: \`${CURRENT_BRANCH}\`
- Tasks completed: ${DONE_TASKS}

## Checklist
- [ ] Review changes
- [ ] Run tests locally if needed

🤖 Created automatically by agent-loop
BODY
)" || echo "Warning: gh pr create failed. Create the PR manually from branch ${CURRENT_BRANCH}."
      else
        echo "No commits ahead of ${BASE_BRANCH}. Skipping PR."
      fi
    fi

    exit 0
  fi

  if [[ "$STATUS" -eq 3 ]]; then
    echo "Dirty worktree detected. Resetting uncommitted changes and retrying."
    git checkout -- .
    continue
  fi

  if [[ "$MAX_RUNS" != "0" && "$RUN_COUNT" -ge "$MAX_RUNS" ]]; then
    echo "Reached MAX_RUNS=${MAX_RUNS}. Exiting."
    exit 0
  fi

  echo "Sleeping for ${LOOP_INTERVAL_SECONDS} seconds."
  sleep "$LOOP_INTERVAL_SECONDS"
done
