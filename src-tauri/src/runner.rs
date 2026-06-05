use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::errors::{WorkerError, WorkerResult};
use crate::git;
use crate::github;
use crate::models::{Project, QueuedTask, RunLogEvent, RunUpdatedEvent};
use crate::tasks;
use crate::AppState;

pub fn spawn_run(app: AppHandle, project: Project, task: QueuedTask, run_id: i64) {
    thread::spawn(move || {
        if let Err(error) = execute_run(&app, &project, &task, run_id) {
            let _ = fail_run(&app, &project, &task, run_id, &error.to_string());
        }
    });
}

fn execute_run(
    app: &AppHandle,
    project: &Project,
    task: &QueuedTask,
    run_id: i64,
) -> WorkerResult<()> {
    let project_path = Path::new(&project.path);
    log_emit(app, run_id, "system", &format!("Starting {}", task.task_id));

    let branch = git::ensure_work_branch(
        project_path,
        &project.branch_prefix,
        &task.task_id,
        &task.title,
    )?;
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().expect("db lock");
        db::update_run_branch(&conn, run_id, &branch)?;
    }
    log_emit(app, run_id, "system", &format!("Using branch {branch}"));

    let prompt = build_agent_prompt(project, task);
    let agent_status = run_agent(app, run_id, project_path, project, &prompt)?;
    if agent_status != 0 {
        return Err(WorkerError::Message(format!(
            "agent exited with status {agent_status}"
        )));
    }

    log_emit(
        app,
        run_id,
        "system",
        &format!("Running test command: {}", project.test_command),
    );
    let test_status = stream_shell(app, run_id, project_path, &project.test_command)?;
    if test_status != 0 {
        return Err(WorkerError::Message(format!(
            "test command exited with status {test_status}"
        )));
    }

    tasks::set_task_status(project_path, &task.task_id, "done")?;
    let commit_hash = git::commit_all(project_path, &task.task_id)?;
    log_emit(app, run_id, "system", &format!("Committed {commit_hash}"));

    let mut pr_url = None;
    if project.auto_push {
        git::push_branch(project_path, &branch)?;
        log_emit(app, run_id, "system", "Pushed branch to origin");

        let base = git::default_base_branch(project_path);
        let pr = github::create_pr(
            project_path,
            &base,
            &format!("agent: complete {}", task.task_id),
            &format!(
                "## Summary\n\nCompleted `{}` from Local Worker.\n\n## Verification\n\n- `{}` passed\n- Commit: `{}`\n\nGenerated automatically by the home Mac worker.",
                task.task_id, project.test_command, commit_hash
            ),
        )?;
        log_emit(app, run_id, "system", &format!("Created PR {pr}"));
        pr_url = Some(pr);
    }

    tasks::append_done(
        project_path,
        task,
        &project.test_command,
        &commit_hash,
        pr_url.as_deref(),
    )?;

    if let Some(issue_number) = task.issue_number {
        let body = format!(
            "Local Worker completed `{}`.\n\n- Test: `{}` passed\n- Commit: `{}`\n- PR: {}",
            task.task_id,
            project.test_command,
            commit_hash,
            pr_url.as_deref().unwrap_or("not created")
        );
        if let Err(error) = github::comment_issue(project_path, issue_number, &body) {
            log_emit(
                app,
                run_id,
                "system",
                &format!("Issue comment failed: {error}"),
            );
        }
    }

    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().expect("db lock");
        let mut done_task = task.clone();
        done_task.status = "done".to_string();
        db::upsert_task(&conn, project.id, &done_task)?;
        db::finish_run(
            &conn,
            run_id,
            "passed",
            "Agent run and verification succeeded",
            Some(&commit_hash),
            pr_url.as_deref(),
        )?;
    }
    emit_run_update(app, run_id, "passed");
    Ok(())
}

pub fn fail_run(
    app: &AppHandle,
    project: &Project,
    task: &QueuedTask,
    run_id: i64,
    reason: &str,
) -> WorkerResult<()> {
    let project_path = Path::new(&project.path);
    log_emit(app, run_id, "system", &format!("Run failed: {reason}"));

    if let Err(error) = tasks::set_task_status(project_path, &task.task_id, "blocked") {
        log_emit(
            app,
            run_id,
            "system",
            &format!("Could not mark task blocked: {error}"),
        );
    }
    if let Err(error) = tasks::append_failed(project_path, task, &project.test_command, reason) {
        log_emit(
            app,
            run_id,
            "system",
            &format!("Could not append failure report: {error}"),
        );
    }
    if let Some(issue_number) = task.issue_number {
        let body = format!(
            "Local Worker could not complete `{}`.\n\n- Test command: `{}`\n- Failure: {}\n\nThe Mac worker left the worktree intact for inspection.",
            task.task_id, project.test_command, reason
        );
        if let Err(error) = github::comment_issue(project_path, issue_number, &body) {
            log_emit(
                app,
                run_id,
                "system",
                &format!("Issue comment failed: {error}"),
            );
        }
    }

    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().expect("db lock");
        let mut failed_task = task.clone();
        failed_task.status = "blocked".to_string();
        db::upsert_task(&conn, project.id, &failed_task)?;
        db::finish_run(&conn, run_id, "failed", reason, None, None)?;
    }
    emit_run_update(app, run_id, "failed");
    Ok(())
}

fn run_agent(
    app: &AppHandle,
    run_id: i64,
    project_path: &Path,
    project: &Project,
    prompt: &str,
) -> WorkerResult<i32> {
    match project.agent_backend.as_str() {
        "claude" => {
            let mut command = Command::new("claude");
            command.args([
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--permission-mode",
                "auto",
                prompt,
            ]);
            stream_command(app, run_id, project_path, &mut command)
        }
        _ => {
            let mut command = Command::new("codex");
            command.args([
                "exec",
                "--json",
                "-C",
                &project.path,
                "--sandbox",
                "workspace-write",
                "--ask-for-approval",
                "never",
                prompt,
            ]);
            stream_command(app, run_id, project_path, &mut command)
        }
    }
}

fn stream_shell(
    app: &AppHandle,
    run_id: i64,
    project_path: &Path,
    shell_command: &str,
) -> WorkerResult<i32> {
    let mut command = Command::new("sh");
    command.args(["-lc", shell_command]);
    stream_command(app, run_id, project_path, &mut command)
}

fn stream_command(
    app: &AppHandle,
    run_id: i64,
    project_path: &Path,
    command: &mut Command,
) -> WorkerResult<i32> {
    let mut child = command
        .current_dir(project_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| WorkerError::Message("child stdout unavailable".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| WorkerError::Message("child stderr unavailable".to_string()))?;

    let app_out = app.clone();
    let out_thread = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            log_emit(&app_out, run_id, "stdout", &line);
        }
    });

    let app_err = app.clone();
    let err_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            log_emit(&app_err, run_id, "stderr", &line);
        }
    });

    let status = child.wait()?;
    let _ = out_thread.join();
    let _ = err_thread.join();
    Ok(status.code().unwrap_or(1))
}

pub fn log_emit(app: &AppHandle, run_id: i64, stream: &str, line: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(conn) = state.db.lock() {
            let _ = db::append_log(&conn, run_id, stream, line);
        }
    }
    let _ = app.emit(
        "run-log",
        RunLogEvent {
            run_id,
            stream: stream.to_string(),
            line: line.to_string(),
        },
    );
}

fn emit_run_update(app: &AppHandle, run_id: i64, status: &str) {
    let _ = app.emit(
        "run-updated",
        RunUpdatedEvent {
            run_id,
            status: status.to_string(),
        },
    );
}

fn build_agent_prompt(project: &Project, task: &QueuedTask) -> String {
    format!(
        r#"You are running as Local Worker on a home Mac.

Work only on this queued task:
- Task ID: {task_id}
- Title: {title}
- Issue: {issue}

Required workflow:
1. Read AGENTS.md and tasks/queue.md.
2. Implement the smallest coherent change that satisfies the task goal.
3. Do not commit, push, create a pull request, or edit secrets.
4. Do not run destructive cleanup. Leave verification and Git operations to the Local Worker app.
5. In your final message, summarize changed files, risks, and any tests you ran manually.

Configured verification command that the app will run after you finish:
{test_command}
"#,
        task_id = task.task_id,
        title = task.title,
        issue = task
            .issue_number
            .map(|value| format!("#{value}"))
            .unwrap_or_else(|| "none".to_string()),
        test_command = project.test_command
    )
}
