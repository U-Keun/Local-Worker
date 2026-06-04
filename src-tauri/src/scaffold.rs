use std::fs;
use std::path::Path;
use std::process::Command;

use crate::errors::{WorkerError, WorkerResult};
use crate::git;
use crate::models::{CreateProjectRequest, ProjectCreationPreview, ProjectSetupCheck};
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
    write_scaffold_files(&project_path, &name, &request.normalized_test_command())?;
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

pub fn preview_project_creation(request: &CreateProjectRequest) -> ProjectCreationPreview {
    let name_result = normalize_project_name(&request.name);
    let project_name = match &name_result {
        Ok(name) => name.clone(),
        Err(_) => fallback_project_name(&request.name),
    };
    let parent = Path::new(request.parent_path.trim());
    let project_path = parent.join(&project_name);
    let backend = request.normalized_agent_backend();

    let mut checks = Vec::new();
    match name_result {
        Ok(_) => checks.push(setup_check(
            "name",
            "Project name",
            "passed",
            "Name can be used as a folder and repository name.",
        )),
        Err(error) => checks.push(setup_check(
            "name",
            "Project name",
            "failed",
            &error.to_string(),
        )),
    }

    if parent.exists() && parent.is_dir() {
        checks.push(setup_check(
            "parent",
            "Parent folder",
            "passed",
            &format!("{} is available.", parent.display()),
        ));
    } else {
        checks.push(setup_check(
            "parent",
            "Parent folder",
            "failed",
            &format!("{} is not an available folder.", parent.display()),
        ));
    }

    if project_path.exists() {
        checks.push(setup_check(
            "target",
            "Target folder",
            "failed",
            &format!("{} already exists.", project_path.display()),
        ));
    } else {
        checks.push(setup_check(
            "target",
            "Target folder",
            "passed",
            &format!("{} is free.", project_path.display()),
        ));
    }

    if request.create_github_repo {
        checks.push(setup_check(
            "github",
            "GitHub CLI",
            if git::command_exists("gh") {
                "passed"
            } else {
                "failed"
            },
            if git::command_exists("gh") {
                "gh is available for repository creation."
            } else {
                "gh is required to create a GitHub repository."
            },
        ));
    } else {
        checks.push(setup_check(
            "github",
            "GitHub repo",
            "warning",
            "A local git repository will be created without an origin remote.",
        ));
    }

    let backend_available = git::command_exists(&backend);
    let backend_detail = if backend_available {
        format!("{backend} is available on this Mac.")
    } else {
        format!("{backend} is not detected yet.")
    };
    checks.push(setup_check(
        "backend",
        "Agent backend",
        if backend_available {
            "passed"
        } else {
            "warning"
        },
        &backend_detail,
    ));

    checks.push(setup_check(
        "test-command",
        "Test command",
        "passed",
        &format!(
            "`{}` will be stored for verification.",
            request.normalized_test_command()
        ),
    ));

    ProjectCreationPreview {
        project_name,
        path: project_path.to_string_lossy().to_string(),
        repo_name: request.create_github_repo.then(|| {
            project_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("local-worker-project")
                .to_string()
        }),
        checks,
        files: scaffold_file_list(),
    }
}

pub fn check_project_setup(project_path: &Path) -> Vec<ProjectSetupCheck> {
    let mut checks = Vec::new();
    checks.push(if project_path.exists() {
        setup_check(
            "path",
            "Project folder",
            "passed",
            &format!("{} exists.", project_path.display()),
        )
    } else {
        setup_check(
            "path",
            "Project folder",
            "failed",
            &format!("{} does not exist.", project_path.display()),
        )
    });

    checks.push(if project_path.is_dir() {
        setup_check("directory", "Directory", "passed", "Folder can be opened.")
    } else {
        setup_check(
            "directory",
            "Directory",
            "failed",
            "Path is not a directory.",
        )
    });

    checks.push(if project_path.join(".git").exists() {
        setup_check("git", "Git repository", "passed", ".git folder is present.")
    } else {
        setup_check(
            "git",
            "Git repository",
            "failed",
            "Register a git repository.",
        )
    });

    checks.push(match git::repo_slug(project_path) {
        Some(repo) => setup_check("remote", "Origin remote", "passed", &repo),
        None => setup_check(
            "remote",
            "Origin remote",
            "warning",
            "No GitHub origin remote was detected.",
        ),
    });

    let required = [
        "AGENTS.md",
        "tasks/queue.md",
        "tasks/done.md",
        "tasks/failed.md",
    ];
    let missing = required
        .iter()
        .filter(|file| !project_path.join(file).exists())
        .copied()
        .collect::<Vec<_>>();
    checks.push(if missing.is_empty() {
        setup_check(
            "tasks",
            "Task files",
            "passed",
            "Agent contract and task files exist.",
        )
    } else {
        setup_check(
            "tasks",
            "Task files",
            "warning",
            &format!("Missing: {}", missing.join(", ")),
        )
    });

    checks.push(match git::dirty_non_task_files(project_path) {
        Ok(dirty) if dirty.is_empty() => setup_check(
            "worktree",
            "Worktree",
            "passed",
            "No non-task changes detected.",
        ),
        Ok(dirty) => setup_check(
            "worktree",
            "Worktree",
            "warning",
            &format!("Uncommitted non-task changes: {}", dirty.join(", ")),
        ),
        Err(error) => setup_check("worktree", "Worktree", "warning", &error.to_string()),
    });

    checks.push(cli_check("gh", "GitHub CLI"));
    checks.push(cli_check("codex", "Codex CLI"));
    checks.push(cli_check("claude", "Claude CLI"));

    if git::command_exists("gh") {
        checks.push(if gh_auth_available() {
            setup_check("gh-auth", "GitHub auth", "passed", "gh auth is available.")
        } else {
            setup_check(
                "gh-auth",
                "GitHub auth",
                "warning",
                "Run gh auth login before syncing.",
            )
        });
    }

    checks
}

fn write_scaffold_files(project_path: &Path, name: &str, test_command: &str) -> WorkerResult<()> {
    fs::write(
        project_path.join("README.md"),
        format!(
            "# {name}\n\nThis project is prepared for Local Worker automation.\n\n## Verification\n\n```bash\n{test_command}\n```\n"
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

pub(crate) fn normalize_project_name(name: &str) -> WorkerResult<String> {
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

fn fallback_project_name(name: &str) -> String {
    name.trim()
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
        .chars()
        .take(40)
        .collect::<String>()
        .if_empty("local-worker-project")
}

fn scaffold_file_list() -> Vec<String> {
    vec![
        "README.md".to_string(),
        ".gitignore".to_string(),
        "AGENTS.md".to_string(),
        "tasks/queue.md".to_string(),
        "tasks/done.md".to_string(),
        "tasks/failed.md".to_string(),
        "scripts/smoke-test.sh".to_string(),
    ]
}

fn cli_check(program: &str, label: &str) -> ProjectSetupCheck {
    if git::command_exists(program) {
        setup_check(
            program,
            label,
            "passed",
            &format!("{program} is available."),
        )
    } else {
        setup_check(
            program,
            label,
            "warning",
            &format!("{program} is not detected."),
        )
    }
}

fn setup_check(id: &str, label: &str, status: &str, detail: &str) -> ProjectSetupCheck {
    ProjectSetupCheck {
        id: id.to_string(),
        label: label.to_string(),
        status: status.to_string(),
        detail: detail.to_string(),
    }
}

fn gh_auth_available() -> bool {
    Command::new("gh")
        .args(["auth", "status"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
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
