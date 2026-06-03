#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

if ! command -v claude >/dev/null 2>&1; then
  echo "claude CLI not found. Install Claude Code first."
  exit 127
fi

if [[ -f ".agent.env" ]]; then
  echo ".agent.env already exists."
  read -rp "Overwrite it? [y/N] " answer
  [[ "$answer" =~ ^[Yy]$ ]] || exit 0
fi

claude "You are helping set up this local dev agent template for the first time.

Your job: ask the user a few short questions to configure .agent.env, then write the file.

Steps:
1. Read .agent.env.example so you understand all available options and their defaults.
2. Ask the user about each key setting, one question at a time:
   - TEST_COMMAND: the shell command that verifies the project (e.g. 'npm test', 'cargo test', 'bash scripts/smoke-test.sh')
   - CLAUDE_MODEL: which Claude model to use for agent runs (claude-opus-4-7 / claude-sonnet-4-6 / claude-haiku-4-5-20251001, or leave blank for the Claude Code default)
   - NOTIFY: send a desktop notification when a run completes or fails? (yes/no)
   - CREATE_BRANCH: automatically create a new branch when on main/master? (yes/no)
   - CLAUDE_TIMEOUT_SECONDS: seconds before a Claude run is killed (default 600, 0 = no timeout)
   - LOOP_INTERVAL_SECONDS: seconds between runs when using agent-loop.sh (default 1800)
3. For each question, briefly explain what the setting does before asking (one sentence max).
4. After all questions are answered, write .agent.env based on .agent.env.example with the user's chosen values filled in.
5. Confirm the file was written and show a short summary of the chosen settings.

Start by reading .agent.env.example, then ask the first question."
