use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::errors::WorkerResult;
use crate::models::{LogLine, Project, ProjectUpdate, QueuedTask, RunRecord};

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
            project.poll_interval_seconds,
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
}
