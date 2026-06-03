#!/usr/bin/env bash
set -euo pipefail

required_files=(
  "README.md"
  "CLAUDE.md"
  "AGENTS.md"
  "SECURITY.md"
  ".agent.env.example"
  "tasks/queue.md"
  "tasks/done.md"
  "tasks/failed.md"
  "scripts/agent-once.sh"
  "scripts/agent-loop.sh"
)

for file in "${required_files[@]}"; do
  if [[ ! -f "$file" ]]; then
    echo "Missing required file: $file"
    exit 1
  fi
done

bash -n scripts/agent-once.sh
bash -n scripts/agent-loop.sh

if ! grep -q "Status: open" tasks/queue.md; then
  echo "tasks/queue.md should contain at least one open task for the initial template."
  exit 1
fi

echo "Smoke test passed."
