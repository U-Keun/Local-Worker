mod autostart;
mod db;
mod errors;
mod git;
mod github;
mod models;
mod runner;
mod scaffold;
mod tasks;

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rusqlite::Connection;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::errors::{WorkerError, WorkerResult};
use crate::models::{
    AppStatus, CreateProjectRequest, Project, ProjectCreationPreview, ProjectSetupCheck,
    ProjectUpdate, QueuedTask, RunRecord, SyncResult, WorkerHealth,
};

pub struct AppState {
    pub db: Mutex<Connection>,
    poller_running: Arc<AtomicBool>,
}

impl AppState {
    fn new(db: Connection) -> Self {
        Self {
            db: Mutex::new(db),
            poller_running: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[tauri::command]
fn app_status() -> AppStatus {
    AppStatus {
        codex_available: git::command_exists("codex"),
        claude_available: git::command_exists("claude"),
        gh_available: git::command_exists("gh"),
        autostart_enabled: autostart::is_enabled(),
    }
}

#[tauri::command]
fn check_worker_health(state: tauri::State<'_, AppState>) -> Result<WorkerHealth, String> {
    let status = app_status();
    let polling_active = state.poller_running.load(Ordering::SeqCst);
    let (projects, last_sync_at, next_sync, latest_run, latest_sync_error) = {
        let conn = state.db.lock().expect("db lock");
        (
            db::list_projects(&conn).map_err(String::from)?,
            db::last_issue_sync_at(&conn).map_err(String::from)?,
            db::next_project_check_at(&conn).map_err(String::from)?,
            db::latest_run(&conn).map_err(String::from)?,
            db::latest_sync_error(&conn).map_err(String::from)?,
        )
    };

    let mut open_task_count = 0usize;
    let mut running_count = 0usize;
    let mut failed_run_count = 0usize;
    {
        let conn = state.db.lock().expect("db lock");
        for project in &projects {
            if let Ok(tasks) = tasks::read_tasks(Path::new(&project.path)) {
                open_task_count += tasks.iter().filter(|task| task.status == "open").count();
            }
            if let Ok(runs) = db::list_runs(&conn, project.id) {
                running_count += runs.iter().filter(|run| run.status == "running").count();
                failed_run_count += runs.iter().filter(|run| run.status == "failed").count();
            }
        }
    }

    let missing_backend = !status.codex_available && !status.claude_available;
    let (primary_action, intervention_reason) = if projects.is_empty() {
        (
            "Complete setup".to_string(),
            Some("No project is registered yet.".to_string()),
        )
    } else if !status.gh_available {
        (
            "Complete setup".to_string(),
            Some("gh CLI is required for issue sync and PR creation.".to_string()),
        )
    } else if missing_backend {
        (
            "Complete setup".to_string(),
            Some("Install Codex or Claude CLI before running tasks.".to_string()),
        )
    } else if let Some(error) = latest_sync_error {
        (
            "Sync now".to_string(),
            Some(format!("Issue sync needs attention: {error}")),
        )
    } else if failed_run_count > 0
        || latest_run
            .as_ref()
            .is_some_and(|run| run.status == "failed")
    {
        (
            "Review failure".to_string(),
            Some("A recent run needs attention.".to_string()),
        )
    } else if open_task_count > 0 {
        ("Run next".to_string(), None)
    } else {
        ("Sync now".to_string(), None)
    };

    Ok(WorkerHealth {
        polling_active,
        project_count: projects.len(),
        running_count,
        open_task_count,
        failed_run_count,
        last_sync_at,
        next_sync: polling_active.then_some(next_sync).flatten(),
        codex_available: status.codex_available,
        claude_available: status.claude_available,
        gh_available: status.gh_available,
        autostart_enabled: status.autostart_enabled,
        needs_attention: intervention_reason.is_some(),
        intervention_reason,
        primary_action,
    })
}

#[tauri::command]
fn check_project_setup(path: String) -> Vec<ProjectSetupCheck> {
    scaffold::check_project_setup(Path::new(path.trim()))
}

#[tauri::command]
fn preview_project_creation(request: CreateProjectRequest) -> ProjectCreationPreview {
    scaffold::preview_project_creation(&request)
}

#[tauri::command]
fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<Project>, String> {
    let conn = state.db.lock().expect("db lock");
    db::list_projects(&conn).map_err(Into::into)
}

#[tauri::command]
fn add_project(path: String, state: tauri::State<'_, AppState>) -> Result<Project, String> {
    let project_path = PathBuf::from(path.trim());
    if !project_path.exists() {
        return Err(format!(
            "project path does not exist: {}",
            project_path.display()
        ));
    }
    if !project_path.join(".git").exists() {
        return Err(format!("not a git repository: {}", project_path.display()));
    }
    tasks::ensure_task_files(&project_path).map_err(String::from)?;
    let name = project_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Project")
        .to_string();
    let repo = git::repo_slug(&project_path);
    let conn = state.db.lock().expect("db lock");
    db::insert_project(
        &conn,
        &name,
        &project_path.to_string_lossy(),
        repo.as_deref(),
    )
    .map_err(Into::into)
}

#[tauri::command]
fn create_project(
    request: CreateProjectRequest,
    state: tauri::State<'_, AppState>,
) -> Result<Project, String> {
    let (path, created_repo) = scaffold::create_project(&request).map_err(String::from)?;
    let project_path = PathBuf::from(&path);
    let name = project_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Project")
        .to_string();
    let repo = created_repo.or_else(|| git::repo_slug(&project_path));
    let issue_label = request.normalized_issue_label();
    let test_command = request.normalized_test_command();
    let agent_backend = request.normalized_agent_backend();
    let branch_prefix = request.normalized_branch_prefix();
    let conn = state.db.lock().expect("db lock");
    db::insert_project_with_settings(
        &conn,
        &name,
        &path,
        repo.as_deref(),
        &issue_label,
        &test_command,
        &agent_backend,
        &branch_prefix,
    )
    .map_err(Into::into)
}

#[tauri::command]
fn update_project(
    project: ProjectUpdate,
    state: tauri::State<'_, AppState>,
) -> Result<Project, String> {
    let conn = state.db.lock().expect("db lock");
    db::update_project(&conn, &project).map_err(Into::into)
}

#[tauri::command]
fn list_tasks(
    project_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QueuedTask>, String> {
    let conn = state.db.lock().expect("db lock");
    let project = db::get_project(&conn, project_id).map_err(String::from)?;
    let mut parsed = tasks::read_tasks(Path::new(&project.path)).map_err(String::from)?;
    for task in &mut parsed {
        task.project_id = Some(project_id);
        db::upsert_task(&conn, project_id, task).map_err(String::from)?;
    }
    Ok(parsed)
}

#[tauri::command]
fn list_runs(project_id: i64, state: tauri::State<'_, AppState>) -> Result<Vec<RunRecord>, String> {
    let conn = state.db.lock().expect("db lock");
    db::list_runs(&conn, project_id).map_err(Into::into)
}

#[tauri::command]
fn list_logs(
    run_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<models::LogLine>, String> {
    let conn = state.db.lock().expect("db lock");
    db::list_logs(&conn, run_id).map_err(Into::into)
}

#[tauri::command]
fn sync_github_issues(
    project_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<SyncResult, String> {
    let conn = state.db.lock().expect("db lock");
    let project = db::get_project(&conn, project_id).map_err(String::from)?;
    sync_project_issues(&conn, &project).map_err(Into::into)
}

#[tauri::command]
fn run_next_task(app: AppHandle, project_id: i64) -> Result<RunRecord, String> {
    start_project_run(&app, project_id).map_err(Into::into)
}

#[tauri::command]
fn start_polling(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state
        .poller_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }

    let app_for_thread = app.clone();
    let running = state.poller_running.clone();
    thread::spawn(move || {
        while running.load(Ordering::SeqCst) {
            let sleep_for = match poll_due_projects(&app_for_thread) {
                Ok(duration) => duration,
                Err(error) => {
                    eprintln!("Local Worker poll failed: {error}");
                    Duration::from_secs(5)
                }
            };
            sleep_interruptibly(&running, sleep_for);
        }
    });
    Ok(())
}

#[tauri::command]
fn stop_polling(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.poller_running.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn set_autostart(enabled: bool) -> Result<bool, String> {
    autostart::set_enabled(enabled).map_err(Into::into)
}

fn poll_due_projects(app: &AppHandle) -> WorkerResult<Duration> {
    let due_projects = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().expect("db lock");
        let projects = db::list_projects(&conn)?;
        let now = Utc::now();
        let mut due = Vec::new();
        for project in projects {
            if project_is_due(&conn, &project, now)? {
                due.push(project);
            }
        }
        due
    };

    for project in due_projects {
        let sync_result = {
            let state = app.state::<AppState>();
            let conn = state.db.lock().expect("db lock");
            sync_project_issues(&conn, &project)
        };

        if let Err(error) = sync_result {
            eprintln!(
                "Local Worker issue sync failed for {}: {error}",
                project.name
            );
            continue;
        }

        if project.auto_run {
            let has_running_run = {
                let state = app.state::<AppState>();
                let conn = state.db.lock().expect("db lock");
                db::has_running_run(&conn, project.id)?
            };
            if has_running_run {
                continue;
            }

            let maybe_task = tasks::first_open_task(Path::new(&project.path))?;
            if maybe_task.is_some() {
                if let Err(error) = start_project_run(app, project.id) {
                    eprintln!("Local Worker run start failed: {error}");
                }
            }
        }
    }

    next_poll_sleep(app)
}

fn sync_project_issues(conn: &Connection, project: &Project) -> WorkerResult<SyncResult> {
    let checked_at = Utc::now();
    let checked_at_text = checked_at.to_rfc3339();
    let next_check_at = next_check_at(project, checked_at).to_rfc3339();

    let result = if git::command_exists("gh") {
        github::sync_issues(conn, project)
    } else {
        Err(WorkerError::Message("gh CLI is not available".to_string()))
    };

    match result {
        Ok(sync_result) => {
            db::record_project_sync_success(
                conn,
                project.id,
                &checked_at_text,
                &next_check_at,
                sync_result.issues_seen,
                sync_result.added,
            )?;
            Ok(sync_result)
        }
        Err(error) => {
            db::record_project_sync_failure(
                conn,
                project.id,
                &checked_at_text,
                &next_check_at,
                &error.to_string(),
            )?;
            Err(error)
        }
    }
}

fn project_is_due(conn: &Connection, project: &Project, now: DateTime<Utc>) -> WorkerResult<bool> {
    let Some(state) = db::get_project_sync_state(conn, project.id)? else {
        return Ok(true);
    };
    let Some(next_check_at) = state.next_check_at.as_deref() else {
        return Ok(true);
    };
    Ok(match parse_rfc3339(next_check_at) {
        Some(due_at) => due_at <= now,
        None => true,
    })
}

fn next_poll_sleep(app: &AppHandle) -> WorkerResult<Duration> {
    let now = Utc::now();
    let mut shortest: Option<Duration> = None;
    let state = app.state::<AppState>();
    let conn = state.db.lock().expect("db lock");
    let projects = db::list_projects(&conn)?;

    if projects.is_empty() {
        return Ok(Duration::from_secs(5));
    }

    for project in projects {
        let wait = match db::get_project_sync_state(&conn, project.id)? {
            Some(sync_state) => match sync_state.next_check_at.as_deref().and_then(parse_rfc3339) {
                Some(next_at) if next_at > now => (next_at - now)
                    .to_std()
                    .unwrap_or_else(|_| Duration::from_secs(1)),
                _ => Duration::from_secs(1),
            },
            None => Duration::from_secs(1),
        };
        shortest = Some(shortest.map_or(wait, |current| current.min(wait)));
    }

    Ok(shortest
        .unwrap_or_else(|| Duration::from_secs(5))
        .clamp(Duration::from_secs(1), Duration::from_secs(30)))
}

fn next_check_at(project: &Project, checked_at: DateTime<Utc>) -> DateTime<Utc> {
    checked_at + ChronoDuration::seconds(project.safe_poll_interval_seconds())
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn sleep_interruptibly(running: &AtomicBool, duration: Duration) {
    let duration = duration.clamp(Duration::from_secs(1), Duration::from_secs(30));
    let mut slept = Duration::ZERO;
    while running.load(Ordering::SeqCst) && slept < duration {
        let step = Duration::from_secs(1).min(duration - slept);
        thread::sleep(step);
        slept += step;
    }
}

fn start_project_run(app: &AppHandle, project_id: i64) -> WorkerResult<RunRecord> {
    let state = app.state::<AppState>();
    let (project, task, run) = {
        let conn = state.db.lock().expect("db lock");
        let project = db::get_project(&conn, project_id)?;
        if db::has_running_run(&conn, project_id)? {
            return Err(WorkerError::Message(
                "a run is already active for this project".to_string(),
            ));
        }

        let dirty = git::dirty_non_task_files(Path::new(&project.path))?;
        if !dirty.is_empty() {
            return Err(WorkerError::Message(format!(
                "worktree has uncommitted non-task changes: {}",
                dirty.join(", ")
            )));
        }

        let mut task = tasks::first_open_task(Path::new(&project.path))?
            .ok_or_else(|| WorkerError::Message("no open tasks found".to_string()))?;
        task.project_id = Some(project_id);
        task.status = "in-progress".to_string();
        tasks::set_task_status(Path::new(&project.path), &task.task_id, "in-progress")?;
        db::upsert_task(&conn, project_id, &task)?;
        let run = db::insert_run(
            &conn,
            project_id,
            Some(&task.task_id),
            &project.agent_backend,
            &project.test_command,
        )?;
        (project, task, run)
    };

    runner::spawn_run(app.clone(), project, task, run.id);
    Ok(run)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("local-worker.sqlite");
            let conn =
                db::open_database(&db_path).map_err(|error| tauri::Error::Anyhow(error.into()))?;
            app.manage(AppState::new(conn));
            setup_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            check_worker_health,
            check_project_setup,
            preview_project_creation,
            list_projects,
            add_project,
            create_project,
            update_project,
            list_tasks,
            list_runs,
            list_logs,
            sync_github_issues,
            run_next_task,
            start_polling,
            stop_polling,
            set_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running Local Worker");
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Local Worker", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::new()
        .tooltip("Local Worker")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::env;
    use std::ffi::OsString;
    use std::fs;

    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct PathGuard {
        original: Option<OsString>,
    }

    impl Drop for PathGuard {
        fn drop(&mut self) {
            if let Some(original) = self.original.as_ref() {
                env::set_var("PATH", original);
            } else {
                env::remove_var("PATH");
            }
        }
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
    }

    #[test]
    fn sync_project_issues_records_success_for_empty_issue_list() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let original_path = env::var_os("PATH");
        let path_guard = PathGuard {
            original: original_path.clone(),
        };
        let temp = tempdir().expect("tempdir");
        let bin_dir = temp.path().join("bin");
        fs::create_dir(&bin_dir).expect("bin dir");
        let fake_gh = bin_dir.join("gh");
        fs::write(
            &fake_gh,
            "#!/bin/sh\nif [ \"$1\" = \"issue\" ] && [ \"$2\" = \"list\" ]; then\n  printf '[]\\n'\n  exit 0\nfi\nexit 1\n",
        )
        .expect("fake gh");
        make_executable(&fake_gh);

        let new_path = match original_path {
            Some(path) => format!("{}:{}", bin_dir.display(), path.to_string_lossy()),
            None => bin_dir.to_string_lossy().to_string(),
        };
        env::set_var("PATH", new_path);

        let project_dir = temp.path().join("project");
        fs::create_dir(&project_dir).expect("project dir");
        let conn = db::open_database(Path::new(":memory:")).expect("db");
        let project = db::insert_project(&conn, "Example", &project_dir.to_string_lossy(), None)
            .expect("project");

        let result = sync_project_issues(&conn, &project).expect("sync");
        assert_eq!(result.issues_seen, 0);
        assert_eq!(result.added, 0);

        let state = db::get_project_sync_state(&conn, project.id)
            .expect("state")
            .expect("state exists");
        assert!(state.last_checked_at.is_some());
        assert!(state.last_success_at.is_some());
        assert!(state.next_check_at.is_some());
        assert!(state.last_error.is_none());

        drop(path_guard);
    }
}
