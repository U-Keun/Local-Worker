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
    exit 0
  fi

  if [[ "$MAX_RUNS" != "0" && "$RUN_COUNT" -ge "$MAX_RUNS" ]]; then
    echo "Reached MAX_RUNS=${MAX_RUNS}. Exiting."
    exit 0
  fi

  echo "Sleeping for ${LOOP_INTERVAL_SECONDS} seconds."
  sleep "$LOOP_INTERVAL_SECONDS"
done
