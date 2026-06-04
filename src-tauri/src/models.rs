use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub repo: Option<String>,
    pub issue_label: String,
    pub poll_interval_seconds: i64,
    pub test_command: String,
    pub agent_backend: String,
    pub branch_prefix: String,
    pub auto_run: bool,
    pub auto_push: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    pub id: Option<i64>,
    pub project_id: Option<i64>,
    pub task_id: String,
    pub issue_number: Option<i64>,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub goal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: i64,
    pub project_id: i64,
    pub task_id: Option<String>,
    pub status: String,
    pub backend: String,
    pub branch: Option<String>,
    pub commit_hash: Option<String>,
    pub pr_url: Option<String>,
    pub test_command: String,
    pub summary: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub id: i64,
    pub run_id: i64,
    pub ts: String,
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub codex_available: bool,
    pub claude_available: bool,
    pub gh_available: bool,
    pub autostart_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub added: usize,
    pub skipped: usize,
    pub issues_seen: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunLogEvent {
    pub run_id: i64,
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunUpdatedEvent {
    pub run_id: i64,
    pub status: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectUpdate {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub repo: Option<String>,
    pub issue_label: String,
    pub poll_interval_seconds: i64,
    pub test_command: String,
    pub agent_backend: String,
    pub branch_prefix: String,
    pub auto_run: bool,
    pub auto_push: bool,
    pub created_at: String,
    pub updated_at: String,
}
