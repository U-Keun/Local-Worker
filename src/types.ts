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

export interface WorkerHealth {
  polling_active: boolean;
  project_count: number;
  running_count: number;
  open_task_count: number;
  failed_run_count: number;
  last_sync_at: string | null;
  next_sync: string | null;
  codex_available: boolean;
  claude_available: boolean;
  gh_available: boolean;
  autostart_enabled: boolean;
  needs_attention: boolean;
  intervention_reason: string | null;
  primary_action: "Complete setup" | "Sync now" | "Run next" | "Review failure";
}

export type SetupCheckStatus = "passed" | "warning" | "failed";

export interface ProjectSetupCheck {
  id: string;
  label: string;
  status: SetupCheckStatus;
  detail: string;
}

export interface ProjectCreationPreview {
  project_name: string;
  path: string;
  repo_name: string | null;
  checks: ProjectSetupCheck[];
  files: string[];
}

export interface SyncResult {
  added: number;
  skipped: number;
  issues_seen: number;
}

export interface CreateProjectRequest {
  name: string;
  parent_path: string;
  create_github_repo: boolean;
  private_repo: boolean;
  test_command?: string;
  agent_backend?: AgentBackend;
  issue_label?: string;
  branch_prefix?: string;
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

export interface ChatSession {
  id: number;
  project_id: number;
  run_id: number | null;
  backend: AgentBackend;
  title: string;
  status: string;
  native_session_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface ChatMessage {
  id: number;
  session_id: number;
  role: "user" | "assistant" | "system";
  content: string;
  created_at: string;
}

export interface ChatTurn {
  id: number;
  session_id: number;
  status: string;
  backend: AgentBackend;
  user_message: string;
  assistant_message: string | null;
  test_command: string;
  test_status: number | null;
  summary: string | null;
  started_at: string;
  finished_at: string | null;
}

export interface CreateChatSessionRequest {
  project_id: number;
  run_id?: number | null;
  backend?: AgentBackend;
  title?: string;
}

export interface SendChatMessageRequest {
  session_id: number;
  content: string;
}

export interface CreateLocalTaskRequest {
  project_id: number;
  source_run_id?: number | null;
  title: string;
  priority: "low" | "medium" | "high";
  goal: string;
  done_criteria: string[];
  constraints: string[];
}

export interface CreateLocalTaskResult {
  task: QueuedTask;
  started_run: RunRecord | null;
  auto_run_status: string;
}

export interface ChatLogEvent {
  turn_id: number;
  stream: string;
  line: string;
}

export interface ChatMessageEvent {
  session_id: number;
  message: ChatMessage;
}

export interface ChatTurnUpdatedEvent {
  turn_id: number;
  session_id: number;
  status: string;
}
