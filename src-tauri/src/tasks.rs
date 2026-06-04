use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::errors::{WorkerError, WorkerResult};
use crate::models::QueuedTask;

pub fn task_file(project_path: &Path) -> PathBuf {
    project_path.join("tasks").join("queue.md")
}

pub fn done_file(project_path: &Path) -> PathBuf {
    project_path.join("tasks").join("done.md")
}

pub fn failed_file(project_path: &Path) -> PathBuf {
    project_path.join("tasks").join("failed.md")
}

pub fn ensure_task_files(project_path: &Path) -> WorkerResult<()> {
    let tasks_dir = project_path.join("tasks");
    fs::create_dir_all(&tasks_dir)?;
    let queue = task_file(project_path);
    if !queue.exists() {
        fs::write(
            &queue,
            "# Task Queue\n\nThe agent should pick the first task with `Status: open`.\n",
        )?;
    }
    let done = done_file(project_path);
    if !done.exists() {
        fs::write(
            &done,
            "# Done Tasks\n\nCompleted tasks should be summarized here.\n",
        )?;
    }
    let failed = failed_file(project_path);
    if !failed.exists() {
        fs::write(
            &failed,
            "# Failed Tasks\n\nFailed or blocked tasks should be summarized here.\n",
        )?;
    }
    Ok(())
}

pub fn read_tasks(project_path: &Path) -> WorkerResult<Vec<QueuedTask>> {
    ensure_task_files(project_path)?;
    let content = fs::read_to_string(task_file(project_path))?;
    Ok(parse_tasks(&content))
}

pub fn first_open_task(project_path: &Path) -> WorkerResult<Option<QueuedTask>> {
    Ok(read_tasks(project_path)?
        .into_iter()
        .find(|task| task.status == "open"))
}

pub fn append_issue_task(
    project_path: &Path,
    issue_number: i64,
    title: &str,
    goal: &str,
) -> WorkerResult<QueuedTask> {
    ensure_task_files(project_path)?;
    let queue_path = task_file(project_path);
    let content = fs::read_to_string(&queue_path)?;
    let task_id = format!("TODO-{issue_number:03}");
    if content.contains(&format!("## {task_id}:")) {
        return parse_tasks(&content)
            .into_iter()
            .find(|task| task.task_id == task_id)
            .ok_or_else(|| {
                WorkerError::Message(format!("{task_id} already exists but could not be parsed"))
            });
    }

    let entry = format!(
        "\n\n## {task_id}: {title}\nStatus: open\nPriority: medium\nIssue: #{issue_number}\n\nGoal:\n{goal}\n\nDone criteria:\n- Task satisfies the goal described in the issue.\n\nConstraints:\n- Keep the diff small.\n- Do not modify unrelated files.\n"
    );
    fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&queue_path)?
        .write_all_str(&entry)?;

    Ok(QueuedTask {
        id: None,
        project_id: None,
        task_id,
        issue_number: Some(issue_number),
        title: title.to_string(),
        status: "open".to_string(),
        priority: "medium".to_string(),
        goal: goal.to_string(),
    })
}

pub fn set_task_status(project_path: &Path, task_id: &str, status: &str) -> WorkerResult<()> {
    let queue_path = task_file(project_path);
    let content = fs::read_to_string(&queue_path)?;
    let mut in_target = false;
    let mut replaced = false;
    let mut output = Vec::new();

    for line in content.lines() {
        if line.starts_with("## TODO-") {
            in_target = line.starts_with(&format!("## {task_id}:"));
        }
        if in_target && line.starts_with("Status: ") {
            output.push(format!("Status: {status}"));
            replaced = true;
        } else {
            output.push(line.to_string());
        }
    }

    if !replaced {
        return Err(WorkerError::Message(format!(
            "task status not found for {task_id}"
        )));
    }

    fs::write(queue_path, format!("{}\n", output.join("\n")))?;
    Ok(())
}

pub fn append_done(
    project_path: &Path,
    task: &QueuedTask,
    test_command: &str,
    commit_hash: &str,
    pr_url: Option<&str>,
) -> WorkerResult<()> {
    let entry = format!(
        "\n\n## {}: {}\nDate: {}\nCommit: {}\n\nSummary:\n- Completed by Local Worker after the configured agent run.\n\nVerification:\n- Command: `{}`\n- Result: passed\n\nFollow-up:\n- PR: {}\n",
        task.task_id,
        task.title,
        Utc::now().date_naive(),
        commit_hash,
        test_command,
        pr_url.unwrap_or("not created")
    );
    fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(done_file(project_path))?
        .write_all_str(&entry)?;
    Ok(())
}

pub fn append_failed(
    project_path: &Path,
    task: &QueuedTask,
    test_command: &str,
    reason: &str,
) -> WorkerResult<()> {
    let entry = format!(
        "\n\n## {}: {}\nDate: {}\n\nAttempted:\n- Local Worker ran the configured agent backend and verification command.\n\nVerification:\n- Command: `{}`\n- Result: failed\n\nFailure reason:\n- {}\n\nSuggested next step:\n- Inspect the latest run log in the Local Worker app and review the worktree before retrying.\n",
        task.task_id,
        task.title,
        Utc::now().date_naive(),
        test_command,
        reason
    );
    fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(failed_file(project_path))?
        .write_all_str(&entry)?;
    Ok(())
}

pub fn parse_tasks(content: &str) -> Vec<QueuedTask> {
    let mut tasks = Vec::new();
    let mut current: Option<QueuedTask> = None;
    let mut section: Option<&str> = None;
    let mut goal_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        if let Some((task_id, title)) = parse_heading(line) {
            if let Some(mut task) = current.take() {
                task.goal = goal_lines.join("\n").trim().to_string();
                tasks.push(task);
            }
            current = Some(QueuedTask {
                id: None,
                project_id: None,
                task_id,
                issue_number: None,
                title,
                status: "open".to_string(),
                priority: "medium".to_string(),
                goal: String::new(),
            });
            goal_lines.clear();
            section = None;
            continue;
        }

        let Some(task) = current.as_mut() else {
            continue;
        };

        if let Some(value) = line.strip_prefix("Status:") {
            task.status = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Priority:") {
            task.priority = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("Issue: #") {
            task.issue_number = value.trim().parse::<i64>().ok();
        } else if line.trim() == "Goal:" {
            section = Some("goal");
        } else if line.trim() == "Done criteria:" || line.trim() == "Constraints:" {
            section = None;
        } else if section == Some("goal") {
            goal_lines.push(line.to_string());
        }
    }

    if let Some(mut task) = current {
        task.goal = goal_lines.join("\n").trim().to_string();
        tasks.push(task);
    }

    tasks
}

fn parse_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## TODO-")?;
    let (number, title) = rest.split_once(':')?;
    Some((format!("TODO-{}", number.trim()), title.trim().to_string()))
}

trait WriteAllStr {
    fn write_all_str(&mut self, content: &str) -> WorkerResult<()>;
}

impl WriteAllStr for fs::File {
    fn write_all_str(&mut self, content: &str) -> WorkerResult<()> {
        use std::io::Write;
        self.write_all(content.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_task_blocks() {
        let tasks = parse_tasks(
            "# Task Queue\n\n## TODO-001: Fix login\nStatus: open\nPriority: high\nIssue: #7\n\nGoal:\nMake login work.\n\nDone criteria:\n- It works\n",
        );

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, "TODO-001");
        assert_eq!(tasks[0].issue_number, Some(7));
        assert_eq!(tasks[0].goal, "Make login work.");
    }

    #[test]
    fn updates_status_in_place() {
        let temp = tempfile::tempdir().expect("tempdir");
        ensure_task_files(temp.path()).expect("files");
        append_issue_task(temp.path(), 4, "Do work", "Goal").expect("append");
        set_task_status(temp.path(), "TODO-004", "in-progress").expect("status");
        let tasks = read_tasks(temp.path()).expect("tasks");
        assert_eq!(tasks[0].status, "in-progress");
    }
}
