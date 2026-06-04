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

use rusqlite::Connection;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

use crate::errors::{WorkerError, WorkerResult};
use crate::models::{
    AppStatus, CreateProjectRequest, Project, ProjectUpdate, QueuedTask, RunRecord, SyncResult,
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
    let conn = state.db.lock().expect("db lock");
    db::insert_project(&conn, &name, &path, repo.as_deref()).map_err(Into::into)
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
    if !git::command_exists("gh") {
        return Err("gh CLI is not available".to_string());
    }
    let conn = state.db.lock().expect("db lock");
    let project = db::get_project(&conn, project_id).map_err(String::from)?;
    github::sync_issues(&conn, &project).map_err(Into::into)
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
            if let Err(error) = poll_once(&app_for_thread) {
                eprintln!("Local Worker poll failed: {error}");
            }
            for _ in 0..60 {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_secs(1));
            }
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

fn poll_once(app: &AppHandle) -> WorkerResult<()> {
    if !git::command_exists("gh") {
        return Ok(());
    }

    let projects = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().expect("db lock");
        db::list_projects(&conn)?
    };

    for project in projects.into_iter().filter(|project| project.auto_run) {
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().expect("db lock");
            if db::has_running_run(&conn, project.id)? {
                continue;
            }
            let _ = github::sync_issues(&conn, &project);
        }

        let maybe_task = tasks::first_open_task(Path::new(&project.path))?;
        if maybe_task.is_some() {
            if let Err(error) = start_project_run(app, project.id) {
                eprintln!("Local Worker run start failed: {error}");
            }
        }
    }
    Ok(())
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
