export type AgentBackend = "codex" | "claude";

export interface Project {
  id: number;
  name: string;
  path: string;
  repo: string | null;
  issue_label: string;
  poll_interval_seconds: number;
  test_command: string;
  agent_backend: AgentBackend;
  branch_prefix: string;
  auto_run: boolean;
  auto_push: boolean;
  created_at: string;
  updated_at: string;
}

export interface QueuedTask {
  id: number | null;
  project_id: number | null;
  task_id: string;
  issue_number: number | null;
  title: string;
  status: string;
  priority: string;
  goal: string;
}

export interface RunRecord {
  id: number;
  project_id: number;
  task_id: string | null;
  status: string;
  backend: AgentBackend;
  branch: string | null;
  commit_hash: string | null;
  pr_url: string | null;
  test_command: string;
  summary: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface LogLine {
  id: number;
  run_id: number;
  ts: string;
  stream: string;
  line: string;
}

export interface AppStatus {
  codex_available: boolean;
  claude_available: boolean;
  gh_available: boolean;
  autostart_enabled: boolean;
}

export interface SyncResult {
  added: number;
  skipped: number;
  issues_seen: number;
}

export interface RunLogEvent {
  run_id: number;
  stream: string;
  line: string;
}

export interface RunUpdatedEvent {
  run_id: number;
  status: string;
}
