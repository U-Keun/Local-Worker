use std::path::Path;
use std::process::Command;

use chrono::Utc;

use crate::errors::{WorkerError, WorkerResult};

pub fn command_exists(program: &str) -> bool {
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {program} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn repo_slug(project_path: &Path) -> Option<String> {
    let output = git_output(project_path, &["config", "--get", "remote.origin.url"]).ok()?;
    parse_repo_slug(output.trim())
}

pub fn parse_repo_slug(remote: &str) -> Option<String> {
    let cleaned = remote.trim().trim_end_matches(".git");
    if let Some(rest) = cleaned.strip_prefix("git@github.com:") {
        return Some(rest.to_string());
    }
    if let Some(rest) = cleaned.strip_prefix("https://github.com/") {
        return Some(rest.to_string());
    }
    if let Some(rest) = cleaned.strip_prefix("ssh://git@github.com/") {
        return Some(rest.to_string());
    }
    None
}

pub fn dirty_non_task_files(project_path: &Path) -> WorkerResult<Vec<String>> {
    let output = git_output(project_path, &["status", "--porcelain"])?;
    let dirty = output
        .lines()
        .filter_map(status_path)
        .filter(|path| !is_task_file(path))
        .collect();
    Ok(dirty)
}

pub fn current_branch(project_path: &Path) -> WorkerResult<String> {
    git_output(project_path, &["branch", "--show-current"]).map(|value| value.trim().to_string())
}

pub fn ensure_work_branch(
    project_path: &Path,
    branch_prefix: &str,
    task_id: &str,
    title: &str,
) -> WorkerResult<String> {
    let current = current_branch(project_path)?;
    if !matches!(current.as_str(), "main" | "master" | "") {
        return Ok(current);
    }

    let slug = slugify(title);
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let branch = format!(
        "{}/{}-{}-{}",
        branch_prefix.trim_matches('/'),
        task_id.to_ascii_lowercase(),
        slug,
        timestamp
    );
    git_output(project_path, &["checkout", "-b", &branch])?;
    Ok(branch)
}

pub fn commit_all(project_path: &Path, task_id: &str) -> WorkerResult<String> {
    git_output(project_path, &["add", "-A"])?;
    let staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(project_path)
        .status()?;
    if staged.success() {
        return Err(WorkerError::Message(
            "no staged changes to commit".to_string(),
        ));
    }
    git_output(
        project_path,
        &["commit", "-m", &format!("agent: complete {task_id}")],
    )?;
    git_output(project_path, &["rev-parse", "--short", "HEAD"])
        .map(|value| value.trim().to_string())
}

pub fn push_branch(project_path: &Path, branch: &str) -> WorkerResult<()> {
    git_output(project_path, &["push", "-u", "origin", branch])?;
    Ok(())
}

pub fn default_base_branch(project_path: &Path) -> String {
    if let Ok(output) = git_output(
        project_path,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        if let Some((_, branch)) = output.trim().split_once('/') {
            return branch.to_string();
        }
    }
    if git_output(project_path, &["rev-parse", "--verify", "main"]).is_ok() {
        "main".to_string()
    } else {
        "master".to_string()
    }
}

pub fn git_output(project_path: &Path, args: &[&str]) -> WorkerResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(WorkerError::Message(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn status_path(line: &str) -> Option<String> {
    let path = line.get(3..)?.trim();
    if let Some((_, right)) = path.split_once(" -> ") {
        Some(right.to_string())
    } else {
        Some(path.to_string())
    }
}

fn is_task_file(path: &str) -> bool {
    matches!(path, "tasks/queue.md" | "tasks/done.md" | "tasks/failed.md")
}

fn slugify(value: &str) -> String {
    let mut slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug.trim_matches('-').chars().take(40).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_remotes() {
        assert_eq!(
            parse_repo_slug("git@github.com:owner/repo.git"),
            Some("owner/repo".to_string())
        );
        assert_eq!(
            parse_repo_slug("https://github.com/owner/repo.git"),
            Some("owner/repo".to_string())
        );
    }
}
