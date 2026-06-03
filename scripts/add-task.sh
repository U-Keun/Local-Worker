#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

usage() {
  echo "Usage: $0 --title <title> --goal <goal> [--issue <number>] [--priority <priority>]"
  exit 1
}

TITLE=""
GOAL=""
ISSUE_NUMBER=""
PRIORITY="medium"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --title)    TITLE="$2";         shift 2 ;;
    --goal)     GOAL="$2";          shift 2 ;;
    --issue)    ISSUE_NUMBER="$2";  shift 2 ;;
    --priority) PRIORITY="$2";      shift 2 ;;
    *) usage ;;
  esac
done

[[ -z "$TITLE" || -z "$GOAL" ]] && usage

TASK_FILE="${TASK_FILE:-tasks/queue.md}"

if [[ -n "$ISSUE_NUMBER" ]]; then
  TASK_ID="TODO-$(printf '%03d' "$ISSUE_NUMBER")"
  if grep -q "^## ${TASK_ID}:" "$TASK_FILE" 2>/dev/null; then
    echo "${TASK_ID} already exists in ${TASK_FILE}. Skipping."
    exit 0
  fi
  ISSUE_LINE="Issue: #${ISSUE_NUMBER}"$'\n'
else
  LAST_NUM=$(grep -oE '^## TODO-[0-9]+' "$TASK_FILE" 2>/dev/null | grep -oE '[0-9]+' | sort -n | tail -1 || echo 0)
  NEXT_NUM=$(( LAST_NUM + 1 ))
  TASK_ID="TODO-$(printf '%03d' "$NEXT_NUM")"
  ISSUE_LINE=""
fi

cat >> "$TASK_FILE" <<ENTRY

## ${TASK_ID}: ${TITLE}
Status: open
Priority: ${PRIORITY}
${ISSUE_LINE}
Goal:
${GOAL}

Done criteria:
- Task satisfies the goal described above.

Constraints:
- Keep the diff small.
- Do not modify unrelated files.
ENTRY

echo "Added ${TASK_ID} to ${TASK_FILE}."
