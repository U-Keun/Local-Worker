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

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub parent_path: String,
    pub create_github_repo: bool,
    pub private_repo: bool,
    pub test_command: Option<String>,
    pub agent_backend: Option<String>,
    pub issue_label: Option<String>,
    pub branch_prefix: Option<String>,
}

impl CreateProjectRequest {
    pub fn normalized_test_command(&self) -> String {
        normalized_optional(&self.test_command, "bash scripts/smoke-test.sh")
    }

    pub fn normalized_agent_backend(&self) -> String {
        match normalized_optional(&self.agent_backend, "codex").as_str() {
            "claude" => "claude".to_string(),
            _ => "codex".to_string(),
        }
    }

    pub fn normalized_issue_label(&self) -> String {
        normalized_optional(&self.issue_label, "agent-task")
    }

    pub fn normalized_branch_prefix(&self) -> String {
        let prefix = normalized_optional(&self.branch_prefix, "codex")
            .trim_matches('/')
            .to_string();
        if prefix.is_empty() {
            "codex".to_string()
        } else {
            prefix
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerHealth {
    pub polling_active: bool,
    pub project_count: usize,
    pub running_count: usize,
    pub open_task_count: usize,
    pub failed_run_count: usize,
    pub last_sync_at: Option<String>,
    pub next_sync: Option<String>,
    pub codex_available: bool,
    pub claude_available: bool,
    pub gh_available: bool,
    pub autostart_enabled: bool,
    pub needs_attention: bool,
    pub intervention_reason: Option<String>,
    pub primary_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSetupCheck {
    pub id: String,
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectCreationPreview {
    pub project_name: String,
    pub path: String,
    pub repo_name: Option<String>,
    pub checks: Vec<ProjectSetupCheck>,
    pub files: Vec<String>,
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

fn normalized_optional(value: &Option<String>, fallback: &str) -> String {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}
