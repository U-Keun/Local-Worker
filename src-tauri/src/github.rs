use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::db;
use crate::errors::{WorkerError, WorkerResult};
use crate::models::{Project, SyncResult};
use crate::tasks;

#[derive(Debug, Deserialize)]
struct Issue {
    number: i64,
    title: String,
    body: Option<String>,
    url: Option<String>,
}

pub fn sync_issues(conn: &rusqlite::Connection, project: &Project) -> WorkerResult<SyncResult> {
    let project_path = Path::new(&project.path);
    tasks::ensure_task_files(project_path)?;

    let output = Command::new("gh")
        .args([
            "issue",
            "list",
            "--label",
            &project.issue_label,
            "--state",
            "open",
            "--json",
            "number,title,body,url",
            "--limit",
            "50",
        ])
        .current_dir(project_path)
        .output()?;

    if !output.status.success() {
        return Err(WorkerError::Message(format!(
            "gh issue list failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let issues: Vec<Issue> = serde_json::from_slice(&output.stdout)?;
    let mut added = 0;
    let mut skipped = 0;

    for issue in &issues {
        let already_seen = db::issue_exists(conn, project.id, issue.number)?;
        let goal = issue.body.as_deref().unwrap_or("").trim();
        let goal = if goal.is_empty() { &issue.title } else { goal };
        let task = tasks::append_issue_task(project_path, issue.number, &issue.title, goal)?;
        db::upsert_task(conn, project.id, &task)?;
        db::record_issue(
            conn,
            project.id,
            issue.number,
            &task.task_id,
            &issue.title,
            issue.url.as_deref(),
        )?;
        if already_seen {
            skipped += 1;
        } else {
            added += 1;
        }
    }

    Ok(SyncResult {
        added,
        skipped,
        issues_seen: issues.len(),
    })
}

pub fn create_pr(project_path: &Path, base: &str, title: &str, body: &str) -> WorkerResult<String> {
    let output = Command::new("gh")
        .args([
            "pr", "create", "--base", base, "--title", title, "--body", body,
        ])
        .current_dir(project_path)
        .output()?;

    if !output.status.success() {
        return Err(WorkerError::Message(format!(
            "gh pr create failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|line| line.starts_with("http://") || line.starts_with("https://"))
        .map(|line| line.trim().to_string())
        .ok_or_else(|| WorkerError::Message("gh pr create did not return a URL".to_string()))
}

pub fn comment_issue(project_path: &Path, issue_number: i64, body: &str) -> WorkerResult<()> {
    let status = Command::new("gh")
        .args([
            "issue",
            "comment",
            &issue_number.to_string(),
            "--body",
            body,
        ])
        .current_dir(project_path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(WorkerError::Message(format!(
            "gh issue comment failed for issue #{issue_number}"
        )))
    }
}
