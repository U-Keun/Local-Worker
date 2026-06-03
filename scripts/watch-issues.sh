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

: "${WATCH_INTERVAL_SECONDS:=60}"
: "${TASK_FILE:=tasks/queue.md}"
: "${ISSUE_LABEL:=agent-task}"

if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI not found. Install GitHub CLI to use watch-issues."
  exit 1
fi

echo "Watching GitHub issues with label '${ISSUE_LABEL}' every ${WATCH_INTERVAL_SECONDS}s."

while true; do
  echo "=== issue watch at $(date) ==="

  ISSUES=$(gh issue list \
    --label "$ISSUE_LABEL" \
    --state open \
    --json number,title,body \
    --limit 50)

  COUNT=$(echo "$ISSUES" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")

  if [[ "$COUNT" -eq 0 ]]; then
    echo "No open issues with label '${ISSUE_LABEL}'."
  else
    echo "$ISSUES" | python3 - <<'EOF'
import sys, json, subprocess, os

issues = json.load(sys.stdin)
task_file = os.environ.get("TASK_FILE", "tasks/queue.md")

for issue in issues:
    number = str(issue["number"])
    task_id = f"TODO-{int(number):03d}"

    try:
        with open(task_file) as f:
            content = f.read()
        if f"## {task_id}:" in content:
            print(f"{task_id} already in queue. Skipping.")
            continue
    except FileNotFoundError:
        pass

    title = issue["title"]
    body = (issue.get("body") or "").strip() or title

    print(f"Adding {task_id}: {title}")
    subprocess.run([
        "bash", "scripts/add-task.sh",
        "--issue", number,
        "--title", title,
        "--goal", body,
    ], check=True)
EOF
  fi

  echo "Sleeping for ${WATCH_INTERVAL_SECONDS}s."
  sleep "$WATCH_INTERVAL_SECONDS"
done
