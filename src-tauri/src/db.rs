use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::errors::WorkerResult;
use crate::models::{
    ChatMessage, ChatSession, ChatTurn, LogLine, Project, ProjectSyncState, ProjectUpdate,
    QueuedTask, RunRecord, MIN_POLL_INTERVAL_SECONDS,
};

pub fn now() -> String {
    Utc::now().to_rfc3339()
}

pub fn open_database(path: &Path) -> WorkerResult<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            repo TEXT,
            issue_label TEXT NOT NULL DEFAULT 'agent-task',
            poll_interval_seconds INTEGER NOT NULL DEFAULT 60,
            test_command TEXT NOT NULL DEFAULT 'bash scripts/smoke-test.sh',
            agent_backend TEXT NOT NULL DEFAULT 'codex',
            branch_prefix TEXT NOT NULL DEFAULT 'codex',
            auto_run INTEGER NOT NULL DEFAULT 1,
            auto_push INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            task_id TEXT NOT NULL,
            issue_number INTEGER,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            priority TEXT NOT NULL,
            goal TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(project_id, task_id),
            FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS issue_syncs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            issue_number INTEGER NOT NULL,
            task_id TEXT NOT NULL,
            title TEXT NOT NULL,
            url TEXT,
            pr_url TEXT,
            last_seen_at TEXT NOT NULL,
            UNIQUE(project_id, issue_number),
            FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS project_sync_states (
            project_id INTEGER PRIMARY KEY,
            last_checked_at TEXT,
            last_success_at TEXT,
            next_check_at TEXT,
            last_error TEXT,
            issues_seen INTEGER NOT NULL DEFAULT 0,
            issues_added INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            task_id TEXT,
            status TEXT NOT NULL,
            backend TEXT NOT NULL,
            branch TEXT,
            commit_hash TEXT,
            pr_url TEXT,
            test_command TEXT NOT NULL,
            summary TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id INTEGER NOT NULL,
            ts TEXT NOT NULL,
            stream TEXT NOT NULL,
            line TEXT NOT NULL,
            FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS chat_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            run_id INTEGER,
            backend TEXT NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            native_session_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY(run_id) REFERENCES runs(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS chat_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS chat_turns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL,
            status TEXT NOT NULL,
            backend TEXT NOT NULL,
            user_message TEXT NOT NULL,
            assistant_message TEXT,
            test_command TEXT NOT NULL,
            test_status INTEGER,
            summary TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            FOREIGN KEY(session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
        );
        ",
    )?;
    Ok(conn)
}

pub fn list_projects(conn: &Connection) -> WorkerResult<Vec<Project>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, name, path, repo, issue_label, poll_interval_seconds, test_command,
               agent_backend, branch_prefix, auto_run, auto_push, created_at, updated_at
        FROM projects
        ORDER BY updated_at DESC, id DESC
        ",
    )?;
    let rows = stmt.query_map([], project_from_row)?;
    collect_rows(rows)
}

pub fn get_project(conn: &Connection, project_id: i64) -> WorkerResult<Project> {
    conn.query_row(
        "
        SELECT id, name, path, repo, issue_label, poll_interval_seconds, test_command,
               agent_backend, branch_prefix, auto_run, auto_push, created_at, updated_at
        FROM projects
        WHERE id = ?1
        ",
        [project_id],
        project_from_row,
    )
    .map_err(Into::into)
}

pub fn insert_project(
    conn: &Connection,
    name: &str,
    path: &str,
    repo: Option<&str>,
) -> WorkerResult<Project> {
    let ts = now();
    conn.execute(
        "
        INSERT INTO projects (
            name, path, repo, issue_label, poll_interval_seconds, test_command,
            agent_backend, branch_prefix, auto_run, auto_push, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, 'agent-task', 60, 'bash scripts/smoke-test.sh',
                'codex', 'codex', 1, 1, ?4, ?4)
        ON CONFLICT(path) DO UPDATE SET
            name = excluded.name,
            repo = excluded.repo,
            updated_at = excluded.updated_at
        ",
        params![name, path, repo, ts],
    )?;
    let id = conn.query_row("SELECT id FROM projects WHERE path = ?1", [path], |row| {
        row.get(0)
    })?;
    get_project(conn, id)
}

pub fn insert_project_with_settings(
    conn: &Connection,
    name: &str,
    path: &str,
    repo: Option<&str>,
    issue_label: &str,
    test_command: &str,
    agent_backend: &str,
    branch_prefix: &str,
) -> WorkerResult<Project> {
    let ts = now();
    conn.execute(
        "
        INSERT INTO projects (
            name, path, repo, issue_label, poll_interval_seconds, test_command,
            agent_backend, branch_prefix, auto_run, auto_push, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, 60, ?5, ?6, ?7, 1, 1, ?8, ?8)
        ON CONFLICT(path) DO UPDATE SET
            name = excluded.name,
            repo = excluded.repo,
            issue_label = excluded.issue_label,
            test_command = excluded.test_command,
            agent_backend = excluded.agent_backend,
            branch_prefix = excluded.branch_prefix,
            updated_at = excluded.updated_at
        ",
        params![
            name,
            path,
            repo,
            issue_label,
            test_command,
            agent_backend,
            branch_prefix,
            ts
        ],
    )?;
    let id = conn.query_row("SELECT id FROM projects WHERE path = ?1", [path], |row| {
        row.get(0)
    })?;
    get_project(conn, id)
}

pub fn update_project(conn: &Connection, project: &ProjectUpdate) -> WorkerResult<Project> {
    let ts = now();
    conn.execute(
        "
        UPDATE projects
        SET name = ?1,
            repo = ?2,
            issue_label = ?3,
            poll_interval_seconds = ?4,
            test_command = ?5,
            agent_backend = ?6,
            branch_prefix = ?7,
            auto_run = ?8,
            auto_push = ?9,
            updated_at = ?10
        WHERE id = ?11
        ",
        params![
            project.name,
            project.repo,
            project.issue_label,
            project.poll_interval_seconds.max(MIN_POLL_INTERVAL_SECONDS),
            project.test_command,
            project.agent_backend,
            project.branch_prefix,
            project.auto_run as i64,
            project.auto_push as i64,
            ts,
            project.id
        ],
    )?;
    get_project(conn, project.id)
}

pub fn upsert_task(conn: &Connection, project_id: i64, task: &QueuedTask) -> WorkerResult<()> {
    let ts = now();
    conn.execute(
        "
        INSERT INTO tasks (
            project_id, task_id, issue_number, title, status, priority, goal, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
        ON CONFLICT(project_id, task_id) DO UPDATE SET
            issue_number = excluded.issue_number,
            title = excluded.title,
            status = excluded.status,
            priority = excluded.priority,
            goal = excluded.goal,
            updated_at = excluded.updated_at
        ",
        params![
            project_id,
            task.task_id,
            task.issue_number,
            task.title,
            task.status,
            task.priority,
            task.goal,
            ts
        ],
    )?;
    Ok(())
}

pub fn issue_exists(conn: &Connection, project_id: i64, issue_number: i64) -> WorkerResult<bool> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT id FROM issue_syncs WHERE project_id = ?1 AND issue_number = ?2",
            params![project_id, issue_number],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

pub fn record_issue(
    conn: &Connection,
    project_id: i64,
    issue_number: i64,
    task_id: &str,
    title: &str,
    url: Option<&str>,
) -> WorkerResult<()> {
    let ts = now();
    conn.execute(
        "
        INSERT INTO issue_syncs (project_id, issue_number, task_id, title, url, last_seen_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(project_id, issue_number) DO UPDATE SET
            title = excluded.title,
            url = excluded.url,
            last_seen_at = excluded.last_seen_at
        ",
        params![project_id, issue_number, task_id, title, url, ts],
    )?;
    Ok(())
}

pub fn insert_run(
    conn: &Connection,
    project_id: i64,
    task_id: Option<&str>,
    backend: &str,
    test_command: &str,
) -> WorkerResult<RunRecord> {
    let ts = now();
    conn.execute(
        "
        INSERT INTO runs (project_id, task_id, status, backend, test_command, started_at)
        VALUES (?1, ?2, 'running', ?3, ?4, ?5)
        ",
        params![project_id, task_id, backend, test_command, ts],
    )?;
    get_run(conn, conn.last_insert_rowid())
}

pub fn update_run_branch(conn: &Connection, run_id: i64, branch: &str) -> WorkerResult<()> {
    conn.execute(
        "UPDATE runs SET branch = ?1 WHERE id = ?2",
        params![branch, run_id],
    )?;
    Ok(())
}

pub fn finish_run(
    conn: &Connection,
    run_id: i64,
    status: &str,
    summary: &str,
    commit_hash: Option<&str>,
    pr_url: Option<&str>,
) -> WorkerResult<()> {
    let ts = now();
    conn.execute(
        "
        UPDATE runs
        SET status = ?1,
            summary = ?2,
            commit_hash = ?3,
            pr_url = ?4,
            finished_at = ?5
        WHERE id = ?6
        ",
        params![status, summary, commit_hash, pr_url, ts, run_id],
    )?;
    Ok(())
}

pub fn get_run(conn: &Connection, run_id: i64) -> WorkerResult<RunRecord> {
    conn.query_row(
        "
        SELECT id, project_id, task_id, status, backend, branch, commit_hash, pr_url,
               test_command, summary, started_at, finished_at
        FROM runs
        WHERE id = ?1
        ",
        [run_id],
        run_from_row,
    )
    .map_err(Into::into)
}

pub fn list_runs(conn: &Connection, project_id: i64) -> WorkerResult<Vec<RunRecord>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, project_id, task_id, status, backend, branch, commit_hash, pr_url,
               test_command, summary, started_at, finished_at
        FROM runs
        WHERE project_id = ?1
        ORDER BY id DESC
        LIMIT 100
        ",
    )?;
    let rows = stmt.query_map([project_id], run_from_row)?;
    collect_rows(rows)
}

pub fn has_running_run(conn: &Connection, project_id: i64) -> WorkerResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM runs WHERE project_id = ?1 AND status = 'running'",
        [project_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn last_issue_sync_at(conn: &Connection) -> WorkerResult<Option<String>> {
    conn.query_row(
        "SELECT MAX(last_success_at) FROM project_sync_states",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

pub fn next_project_check_at(conn: &Connection) -> WorkerResult<Option<String>> {
    conn.query_row(
        "SELECT MIN(next_check_at) FROM project_sync_states WHERE next_check_at IS NOT NULL",
        [],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

pub fn latest_sync_error(conn: &Connection) -> WorkerResult<Option<String>> {
    conn.query_row(
        "
        SELECT last_error
        FROM project_sync_states
        WHERE last_error IS NOT NULL AND last_error <> ''
        ORDER BY last_checked_at DESC
        LIMIT 1
        ",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn get_project_sync_state(
    conn: &Connection,
    project_id: i64,
) -> WorkerResult<Option<ProjectSyncState>> {
    conn.query_row(
        "
        SELECT project_id, last_checked_at, last_success_at, next_check_at,
               last_error, issues_seen, issues_added
        FROM project_sync_states
        WHERE project_id = ?1
        ",
        [project_id],
        sync_state_from_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn record_project_sync_success(
    conn: &Connection,
    project_id: i64,
    checked_at: &str,
    next_check_at: &str,
    issues_seen: usize,
    issues_added: usize,
) -> WorkerResult<()> {
    conn.execute(
        "
        INSERT INTO project_sync_states (
            project_id, last_checked_at, last_success_at, next_check_at,
            last_error, issues_seen, issues_added
        )
        VALUES (?1, ?2, ?2, ?3, NULL, ?4, ?5)
        ON CONFLICT(project_id) DO UPDATE SET
            last_checked_at = excluded.last_checked_at,
            last_success_at = excluded.last_success_at,
            next_check_at = excluded.next_check_at,
            last_error = NULL,
            issues_seen = excluded.issues_seen,
            issues_added = excluded.issues_added
        ",
        params![
            project_id,
            checked_at,
            next_check_at,
            issues_seen as i64,
            issues_added as i64
        ],
    )?;
    Ok(())
}

pub fn record_project_sync_failure(
    conn: &Connection,
    project_id: i64,
    checked_at: &str,
    next_check_at: &str,
    error: &str,
) -> WorkerResult<()> {
    conn.execute(
        "
        INSERT INTO project_sync_states (
            project_id, last_checked_at, last_success_at, next_check_at,
            last_error, issues_seen, issues_added
        )
        VALUES (?1, ?2, NULL, ?3, ?4, 0, 0)
        ON CONFLICT(project_id) DO UPDATE SET
            last_checked_at = excluded.last_checked_at,
            next_check_at = excluded.next_check_at,
            last_error = excluded.last_error,
            issues_seen = 0,
            issues_added = 0
        ",
        params![project_id, checked_at, next_check_at, error],
    )?;
    Ok(())
}

pub fn latest_run(conn: &Connection) -> WorkerResult<Option<RunRecord>> {
    conn.query_row(
        "
        SELECT id, project_id, task_id, status, backend, branch, commit_hash, pr_url,
               test_command, summary, started_at, finished_at
        FROM runs
        ORDER BY id DESC
        LIMIT 1
        ",
        [],
        run_from_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn append_log(conn: &Connection, run_id: i64, stream: &str, line: &str) -> WorkerResult<()> {
    conn.execute(
        "INSERT INTO logs (run_id, ts, stream, line) VALUES (?1, ?2, ?3, ?4)",
        params![run_id, now(), stream, line],
    )?;
    Ok(())
}

pub fn list_logs(conn: &Connection, run_id: i64) -> WorkerResult<Vec<LogLine>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, run_id, ts, stream, line
        FROM logs
        WHERE run_id = ?1
        ORDER BY id ASC
        LIMIT 1000
        ",
    )?;
    let rows = stmt.query_map([run_id], |row| {
        Ok(LogLine {
            id: row.get(0)?,
            run_id: row.get(1)?,
            ts: row.get(2)?,
            stream: row.get(3)?,
            line: row.get(4)?,
        })
    })?;
    collect_rows(rows)
}

pub fn insert_chat_session(
    conn: &Connection,
    project_id: i64,
    run_id: Option<i64>,
    backend: &str,
    title: &str,
    native_session_id: Option<&str>,
) -> WorkerResult<ChatSession> {
    let ts = now();
    conn.execute(
        "
        INSERT INTO chat_sessions (
            project_id, run_id, backend, title, status, native_session_id, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, ?6)
        ",
        params![project_id, run_id, backend, title, native_session_id, ts],
    )?;
    get_chat_session(conn, conn.last_insert_rowid())
}

pub fn get_chat_session(conn: &Connection, session_id: i64) -> WorkerResult<ChatSession> {
    conn.query_row(
        "
        SELECT id, project_id, run_id, backend, title, status, native_session_id,
               created_at, updated_at
        FROM chat_sessions
        WHERE id = ?1
        ",
        [session_id],
        chat_session_from_row,
    )
    .map_err(Into::into)
}

pub fn list_chat_sessions(conn: &Connection, project_id: i64) -> WorkerResult<Vec<ChatSession>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, project_id, run_id, backend, title, status, native_session_id,
               created_at, updated_at
        FROM chat_sessions
        WHERE project_id = ?1
        ORDER BY updated_at DESC, id DESC
        LIMIT 50
        ",
    )?;
    let rows = stmt.query_map([project_id], chat_session_from_row)?;
    collect_rows(rows)
}

pub fn update_chat_session_native_id(
    conn: &Connection,
    session_id: i64,
    native_session_id: &str,
) -> WorkerResult<()> {
    conn.execute(
        "
        UPDATE chat_sessions
        SET native_session_id = ?1,
            updated_at = ?2
        WHERE id = ?3
        ",
        params![native_session_id, now(), session_id],
    )?;
    Ok(())
}

pub fn touch_chat_session(conn: &Connection, session_id: i64) -> WorkerResult<()> {
    conn.execute(
        "UPDATE chat_sessions SET updated_at = ?1 WHERE id = ?2",
        params![now(), session_id],
    )?;
    Ok(())
}

pub fn insert_chat_message(
    conn: &Connection,
    session_id: i64,
    role: &str,
    content: &str,
) -> WorkerResult<ChatMessage> {
    conn.execute(
        "
        INSERT INTO chat_messages (session_id, role, content, created_at)
        VALUES (?1, ?2, ?3, ?4)
        ",
        params![session_id, role, content, now()],
    )?;
    touch_chat_session(conn, session_id)?;
    get_chat_message(conn, conn.last_insert_rowid())
}

pub fn get_chat_message(conn: &Connection, message_id: i64) -> WorkerResult<ChatMessage> {
    conn.query_row(
        "
        SELECT id, session_id, role, content, created_at
        FROM chat_messages
        WHERE id = ?1
        ",
        [message_id],
        chat_message_from_row,
    )
    .map_err(Into::into)
}

pub fn list_chat_messages(conn: &Connection, session_id: i64) -> WorkerResult<Vec<ChatMessage>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, session_id, role, content, created_at
        FROM chat_messages
        WHERE session_id = ?1
        ORDER BY id ASC
        LIMIT 400
        ",
    )?;
    let rows = stmt.query_map([session_id], chat_message_from_row)?;
    collect_rows(rows)
}

pub fn insert_chat_turn(
    conn: &Connection,
    session_id: i64,
    backend: &str,
    user_message: &str,
    test_command: &str,
) -> WorkerResult<ChatTurn> {
    conn.execute(
        "
        INSERT INTO chat_turns (
            session_id, status, backend, user_message, test_command, started_at
        )
        VALUES (?1, 'running', ?2, ?3, ?4, ?5)
        ",
        params![session_id, backend, user_message, test_command, now()],
    )?;
    touch_chat_session(conn, session_id)?;
    get_chat_turn(conn, conn.last_insert_rowid())
}

pub fn get_chat_turn(conn: &Connection, turn_id: i64) -> WorkerResult<ChatTurn> {
    conn.query_row(
        "
        SELECT id, session_id, status, backend, user_message, assistant_message,
               test_command, test_status, summary, started_at, finished_at
        FROM chat_turns
        WHERE id = ?1
        ",
        [turn_id],
        chat_turn_from_row,
    )
    .map_err(Into::into)
}

pub fn list_chat_turns(conn: &Connection, session_id: i64) -> WorkerResult<Vec<ChatTurn>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, session_id, status, backend, user_message, assistant_message,
               test_command, test_status, summary, started_at, finished_at
        FROM chat_turns
        WHERE session_id = ?1
        ORDER BY id DESC
        LIMIT 100
        ",
    )?;
    let rows = stmt.query_map([session_id], chat_turn_from_row)?;
    collect_rows(rows)
}

pub fn finish_chat_turn(
    conn: &Connection,
    turn_id: i64,
    status: &str,
    assistant_message: &str,
    test_status: Option<i64>,
    summary: &str,
) -> WorkerResult<()> {
    let turn = get_chat_turn(conn, turn_id)?;
    conn.execute(
        "
        UPDATE chat_turns
        SET status = ?1,
            assistant_message = ?2,
            test_status = ?3,
            summary = ?4,
            finished_at = ?5
        WHERE id = ?6
        ",
        params![
            status,
            assistant_message,
            test_status,
            summary,
            now(),
            turn_id
        ],
    )?;
    touch_chat_session(conn, turn.session_id)?;
    Ok(())
}

pub fn get_active_chat_turn(conn: &Connection, project_id: i64) -> WorkerResult<Option<ChatTurn>> {
    conn.query_row(
        "
        SELECT ct.id, ct.session_id, ct.status, ct.backend, ct.user_message,
               ct.assistant_message, ct.test_command, ct.test_status, ct.summary,
               ct.started_at, ct.finished_at
        FROM chat_turns ct
        JOIN chat_sessions cs ON cs.id = ct.session_id
        WHERE cs.project_id = ?1 AND ct.status = 'running'
        ORDER BY ct.id DESC
        LIMIT 1
        ",
        [project_id],
        chat_turn_from_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn has_active_chat_turn(conn: &Connection, project_id: i64) -> WorkerResult<bool> {
    Ok(get_active_chat_turn(conn, project_id)?.is_some())
}

fn project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        repo: row.get(3)?,
        issue_label: row.get(4)?,
        poll_interval_seconds: row.get(5)?,
        test_command: row.get(6)?,
        agent_backend: row.get(7)?,
        branch_prefix: row.get(8)?,
        auto_run: row.get::<_, i64>(9)? != 0,
        auto_push: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        status: row.get(3)?,
        backend: row.get(4)?,
        branch: row.get(5)?,
        commit_hash: row.get(6)?,
        pr_url: row.get(7)?,
        test_command: row.get(8)?,
        summary: row.get(9)?,
        started_at: row.get(10)?,
        finished_at: row.get(11)?,
    })
}

fn sync_state_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectSyncState> {
    Ok(ProjectSyncState {
        project_id: row.get(0)?,
        last_checked_at: row.get(1)?,
        last_success_at: row.get(2)?,
        next_check_at: row.get(3)?,
        last_error: row.get(4)?,
        issues_seen: row.get(5)?,
        issues_added: row.get(6)?,
    })
}

fn chat_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChatSession> {
    Ok(ChatSession {
        id: row.get(0)?,
        project_id: row.get(1)?,
        run_id: row.get(2)?,
        backend: row.get(3)?,
        title: row.get(4)?,
        status: row.get(5)?,
        native_session_id: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn chat_message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChatMessage> {
    Ok(ChatMessage {
        id: row.get(0)?,
        session_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn chat_turn_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChatTurn> {
    Ok(ChatTurn {
        id: row.get(0)?,
        session_id: row.get(1)?,
        status: row.get(2)?,
        backend: row.get(3)?,
        user_message: row.get(4)?,
        assistant_message: row.get(5)?,
        test_command: row.get(6)?,
        test_status: row.get(7)?,
        summary: row.get(8)?,
        started_at: row.get(9)?,
        finished_at: row.get(10)?,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> WorkerResult<Vec<T>> {
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_schema_and_inserts_project() {
        let conn = open_database(Path::new(":memory:")).expect("db");
        let project =
            insert_project(&conn, "Example", "/tmp/example", Some("owner/repo")).expect("project");
        assert_eq!(project.issue_label, "agent-task");
        assert_eq!(list_projects(&conn).expect("projects").len(), 1);
    }

    #[test]
    fn records_sync_state_even_when_no_issues_seen() {
        let conn = open_database(Path::new(":memory:")).expect("db");
        let project = insert_project(&conn, "Example", "/tmp/example", None).expect("project");

        record_project_sync_success(
            &conn,
            project.id,
            "2026-01-01T00:00:00Z",
            "2026-01-01T00:01:00Z",
            0,
            0,
        )
        .expect("record sync");

        let state = get_project_sync_state(&conn, project.id)
            .expect("state")
            .expect("state exists");
        assert_eq!(
            state.last_checked_at.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(
            state.last_success_at.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(state.issues_seen, 0);
    }

    #[test]
    fn clamps_poll_interval_on_update() {
        let conn = open_database(Path::new(":memory:")).expect("db");
        let project =
            insert_project(&conn, "Example", "/tmp/example", Some("owner/repo")).expect("project");
        let updated = update_project(
            &conn,
            &ProjectUpdate {
                poll_interval_seconds: 5,
                ..ProjectUpdate::from(project)
            },
        )
        .expect("update");

        assert_eq!(updated.poll_interval_seconds, MIN_POLL_INTERVAL_SECONDS);
    }

    #[test]
    fn creates_chat_session_message_and_turn() {
        let conn = open_database(Path::new(":memory:")).expect("db");
        let project = insert_project(&conn, "Example", "/tmp/example", None).expect("project");
        let session = insert_chat_session(&conn, project.id, None, "codex", "Project chat", None)
            .expect("session");
        let message = insert_chat_message(&conn, session.id, "user", "Hello").expect("message");
        let turn = insert_chat_turn(&conn, session.id, "codex", "Hello", "echo ok").expect("turn");

        assert_eq!(
            list_chat_sessions(&conn, project.id)
                .expect("sessions")
                .len(),
            1
        );
        assert_eq!(
            list_chat_messages(&conn, session.id).expect("messages")[0].id,
            message.id
        );
        assert_eq!(
            list_chat_turns(&conn, session.id).expect("turns")[0].id,
            turn.id
        );
        assert!(has_active_chat_turn(&conn, project.id).expect("active"));
    }

    #[test]
    fn creates_run_linked_chat_session() {
        let conn = open_database(Path::new(":memory:")).expect("db");
        let project = insert_project(&conn, "Example", "/tmp/example", None).expect("project");
        let run = insert_run(&conn, project.id, Some("TODO-001"), "codex", "echo ok").expect("run");
        let session = insert_chat_session(
            &conn,
            project.id,
            Some(run.id),
            "codex",
            "Discuss TODO-001",
            None,
        )
        .expect("session");

        assert_eq!(session.run_id, Some(run.id));
    }
}
