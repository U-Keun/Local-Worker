use std::fs;
use std::path::Path;
use std::process::Command;

use crate::errors::{WorkerError, WorkerResult};
use crate::git;
use crate::models::CreateProjectRequest;
use crate::tasks;

pub fn create_project(request: &CreateProjectRequest) -> WorkerResult<(String, Option<String>)> {
    let name = normalize_project_name(&request.name)?;
    let parent = Path::new(request.parent_path.trim());
    if !parent.exists() {
        return Err(WorkerError::Message(format!(
            "parent path does not exist: {}",
            parent.display()
        )));
    }
    if !parent.is_dir() {
        return Err(WorkerError::Message(format!(
            "parent path is not a directory: {}",
            parent.display()
        )));
    }

    let project_path = parent.join(&name);
    if project_path.exists() {
        return Err(WorkerError::Message(format!(
            "project path already exists: {}",
            project_path.display()
        )));
    }

    fs::create_dir_all(&project_path)?;
    write_scaffold_files(&project_path, &name)?;
    git_output(&project_path, &["init"])?;
    git_output(&project_path, &["add", "-A"])?;
    git_output(
        &project_path,
        &["commit", "-m", "init local worker project"],
    )?;

    let repo = if request.create_github_repo {
        Some(create_github_repo(
            &project_path,
            &name,
            request.private_repo,
        )?)
    } else {
        None
    };

    Ok((project_path.to_string_lossy().to_string(), repo))
}

fn write_scaffold_files(project_path: &Path, name: &str) -> WorkerResult<()> {
    fs::write(
        project_path.join("README.md"),
        format!(
            "# {name}\n\nThis project is prepared for Local Worker automation.\n\n## Verification\n\n```bash\nbash scripts/smoke-test.sh\n```\n"
        ),
    )?;
    fs::write(
        project_path.join(".gitignore"),
        ".DS_Store\n.env\n.env.*\n.agent.env\nlogs/\n",
    )?;
    fs::write(project_path.join("AGENTS.md"), agent_contract())?;

    let scripts = project_path.join("scripts");
    fs::create_dir_all(&scripts)?;
    let smoke_test = scripts.join("smoke-test.sh");
    fs::write(
        &smoke_test,
        "#!/usr/bin/env bash\nset -euo pipefail\n\nrequired_files=(\n  \"README.md\"\n  \"AGENTS.md\"\n  \"tasks/queue.md\"\n  \"tasks/done.md\"\n  \"tasks/failed.md\"\n)\n\nfor file in \"${required_files[@]}\"; do\n  if [[ ! -f \"$file\" ]]; then\n    echo \"Missing required file: $file\"\n    exit 1\n  fi\ndone\n\necho \"Smoke test passed.\"\n",
    )?;
    make_executable(&smoke_test)?;

    tasks::ensure_task_files(project_path)?;
    Ok(())
}

fn create_github_repo(project_path: &Path, name: &str, private_repo: bool) -> WorkerResult<String> {
    if !git::command_exists("gh") {
        return Err(WorkerError::Message("gh CLI is not available".to_string()));
    }

    let visibility = if private_repo {
        "--private"
    } else {
        "--public"
    };
    let output = Command::new("gh")
        .args([
            "repo", "create", name, visibility, "--source", ".", "--remote", "origin", "--push",
        ])
        .current_dir(project_path)
        .output()?;

    if !output.status.success() {
        return Err(WorkerError::Message(format!(
            "gh repo create failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(git::repo_slug(project_path).unwrap_or_else(|| name.to_string()))
}

fn normalize_project_name(name: &str) -> WorkerResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(WorkerError::Message("project name is required".to_string()));
    }

    let normalized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if normalized.is_empty() || normalized == "." || normalized == ".." {
        return Err(WorkerError::Message(
            "project name is not valid".to_string(),
        ));
    }
    Ok(normalized)
}

fn git_output(project_path: &Path, args: &[&str]) -> WorkerResult<String> {
    git::git_output(project_path, args)
}

fn agent_contract() -> &'static str {
    r#"# Agent Contract

This repository is prepared for coding agents running under Local Worker.

## Task Format

Tasks live in `tasks/queue.md`.

```md
## TODO-001: Short task title
Status: open
Priority: medium

Goal:
Describe the intended result.

Done criteria:
- Observable condition 1

Constraints:
- Keep the diff small.
- Do not change unrelated files.
```

Valid statuses:

- `open`
- `in-progress`
- `done`
- `blocked`

## Development Style

Prefer small functions, readable names, simple control flow, direct fixes, and focused tests.

Avoid unrelated formatting churn, hidden dependency changes, broad rewrites, generated files unless required, and public behavior changes without a test or note.

## Local Worker Runner

The Local Worker app owns task status updates, verification, commits, pushes, pull request creation, and GitHub Issue comments. The coding agent should make the smallest coherent change for the selected task and leave Git operations to the app.
"#
}

#[cfg(unix)]
fn make_executable(path: &Path) -> WorkerResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> WorkerResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_project_name() {
        assert_eq!(
            normalize_project_name("My Project!").expect("name"),
            "My-Project"
        );
        assert!(normalize_project_name("   ").is_err());
    }
}
