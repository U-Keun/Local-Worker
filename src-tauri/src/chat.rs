use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::{env, fs};

use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::db;
use crate::errors::{WorkerError, WorkerResult};
use crate::git;
use crate::models::{ChatLogEvent, ChatMessageEvent, ChatSession, ChatTurnUpdatedEvent, Project};
use crate::AppState;

pub fn spawn_chat_turn(app: AppHandle, session: ChatSession, project: Project, turn_id: i64) {
    thread::spawn(move || {
        if let Err(error) = execute_chat_turn(&app, &session, &project, turn_id) {
            let assistant = format!("Agent chat failed: {error}");
            let _ = insert_message_emit(&app, session.id, "assistant", &assistant);
            let _ = finish_turn(
                &app, turn_id, "failed", &assistant, None, &assistant, session.id,
            );
        }
    });
}

fn execute_chat_turn(
    app: &AppHandle,
    session: &ChatSession,
    project: &Project,
    turn_id: i64,
) -> WorkerResult<()> {
    let project_path = Path::new(&project.path);
    let prompt = build_chat_prompt(app, session, project, turn_id)?;
    emit_log(app, turn_id, "system", "Starting agent chat turn");

    let assistant = run_chat_agent(app, turn_id, project_path, session, project, &prompt)?;
    let assistant = if assistant.trim().is_empty() {
        "Agent finished without a final message.".to_string()
    } else {
        assistant
    };

    insert_message_emit(app, session.id, "assistant", assistant.trim())?;
    emit_log(
        app,
        turn_id,
        "system",
        &format!("Running test command: {}", project.test_command),
    );
    let test_status = stream_shell(app, turn_id, project_path, &project.test_command)?;
    let status = if test_status == 0 { "passed" } else { "failed" };
    let summary = if test_status == 0 {
        format!(
            "Agent chat completed and `{}` passed.",
            project.test_command
        )
    } else {
        format!(
            "Agent chat completed but `{}` exited with status {test_status}.",
            project.test_command
        )
    };

    insert_message_emit(app, session.id, "system", &summary)?;
    finish_turn(
        app,
        turn_id,
        status,
        assistant.trim(),
        Some(test_status as i64),
        &summary,
        session.id,
    )?;
    Ok(())
}

fn run_chat_agent(
    app: &AppHandle,
    turn_id: i64,
    project_path: &Path,
    session: &ChatSession,
    project: &Project,
    prompt: &str,
) -> WorkerResult<String> {
    match session.backend.as_str() {
        "claude" => {
            if !git::command_exists("claude") {
                return Err(WorkerError::Message(
                    "claude CLI is not available".to_string(),
                ));
            }
            let native_session_id = session
                .native_session_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            ensure_native_session_id(app, session, &native_session_id)?;
            let mut command = Command::new("claude");
            command.args([
                "-p",
                "--output-format",
                "stream-json",
                "--permission-mode",
                "auto",
                "--session-id",
                &native_session_id,
                prompt,
            ]);
            let raw = stream_command_collect(app, turn_id, project_path, &mut command)?;
            Ok(extract_text_from_json_lines(&raw).unwrap_or(raw))
        }
        _ => {
            if !git::command_exists("codex") {
                return Err(WorkerError::Message(
                    "codex CLI is not available".to_string(),
                ));
            }
            let output_path = last_message_path(turn_id);
            let _ = fs::remove_file(&output_path);
            let output_path_text = output_path.to_string_lossy().to_string();
            let mut command = Command::new("codex");
            if let Some(native_session_id) = session.native_session_id.as_deref() {
                command.args([
                    "exec",
                    "resume",
                    "--json",
                    "--output-last-message",
                    &output_path_text,
                    native_session_id,
                    prompt,
                ]);
            } else {
                command.args([
                    "exec",
                    "--json",
                    "-C",
                    &project.path,
                    "--sandbox",
                    "workspace-write",
                    "--ask-for-approval",
                    "never",
                    "--output-last-message",
                    &output_path_text,
                    prompt,
                ]);
            }
            let raw = stream_command_collect(app, turn_id, project_path, &mut command)?;
            if session.native_session_id.is_none() {
                if let Some(native_session_id) = extract_codex_session_id(&raw) {
                    let _ = ensure_native_session_id(app, session, &native_session_id);
                }
            }
            Ok(fs::read_to_string(&output_path)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(raw))
        }
    }
}

fn stream_shell(
    app: &AppHandle,
    turn_id: i64,
    project_path: &Path,
    shell_command: &str,
) -> WorkerResult<i32> {
    let mut command = Command::new("sh");
    command.args(["-lc", shell_command]);
    stream_command_status(app, turn_id, project_path, &mut command)
}

fn stream_command_status(
    app: &AppHandle,
    turn_id: i64,
    project_path: &Path,
    command: &mut Command,
) -> WorkerResult<i32> {
    let (_, status) = stream_command(app, turn_id, project_path, command)?;
    Ok(status)
}

fn stream_command_collect(
    app: &AppHandle,
    turn_id: i64,
    project_path: &Path,
    command: &mut Command,
) -> WorkerResult<String> {
    let (output, status) = stream_command(app, turn_id, project_path, command)?;
    if status == 0 {
        Ok(output.trim().to_string())
    } else {
        Err(WorkerError::Message(format!(
            "agent exited with status {status}"
        )))
    }
}

fn stream_command(
    app: &AppHandle,
    turn_id: i64,
    project_path: &Path,
    command: &mut Command,
) -> WorkerResult<(String, i32)> {
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
        let mut collected = Vec::new();
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            emit_log(&app_out, turn_id, "stdout", &line);
            collected.push(line);
        }
        collected
    });

    let app_err = app.clone();
    let err_thread = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            emit_log(&app_err, turn_id, "stderr", &line);
        }
    });

    let status = child.wait()?;
    let collected = out_thread.join().unwrap_or_default();
    let _ = err_thread.join();
    Ok((collected.join("\n"), status.code().unwrap_or(1)))
}

fn build_chat_prompt(
    app: &AppHandle,
    session: &ChatSession,
    project: &Project,
    turn_id: i64,
) -> WorkerResult<String> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().expect("db lock");
    let messages = db::list_chat_messages(&conn, session.id)?;
    let turn = db::get_chat_turn(&conn, turn_id)?;
    let recent_messages = messages
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(format!(
        r#"You are chatting inside Local Worker on a home Mac.

Project:
- Name: {name}
- Path: {path}
- Linked run: {run}

Rules:
- You may edit files in the existing worktree and branch.
- Do not commit, push, create pull requests, merge, reset, or discard changes.
- Do not read or print .env, .agent.env, private keys, tokens, credentials, or secret files.
- Keep changes focused on the user's latest message.
- The app will run this verification command after your response:
{test_command}

Recent conversation:
{recent_messages}

Current user message:
{user_message}
"#,
        name = project.name,
        path = project.path,
        run = session
            .run_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        test_command = project.test_command,
        recent_messages = recent_messages,
        user_message = turn.user_message
    ))
}

fn insert_message_emit(
    app: &AppHandle,
    session_id: i64,
    role: &str,
    content: &str,
) -> WorkerResult<()> {
    let message = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().expect("db lock");
        db::insert_chat_message(&conn, session_id, role, content)?
    };
    let _ = app.emit(
        "chat-message",
        ChatMessageEvent {
            session_id,
            message,
        },
    );
    Ok(())
}

fn finish_turn(
    app: &AppHandle,
    turn_id: i64,
    status: &str,
    assistant_message: &str,
    test_status: Option<i64>,
    summary: &str,
    session_id: i64,
) -> WorkerResult<()> {
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().expect("db lock");
        db::finish_chat_turn(
            &conn,
            turn_id,
            status,
            assistant_message,
            test_status,
            summary,
        )?;
    }
    emit_turn_update(app, turn_id, session_id, status);
    Ok(())
}

fn ensure_native_session_id(
    app: &AppHandle,
    session: &ChatSession,
    native_session_id: &str,
) -> WorkerResult<()> {
    if session.native_session_id.is_some() {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let conn = state.db.lock().expect("db lock");
    db::update_chat_session_native_id(&conn, session.id, native_session_id)
}

fn last_message_path(turn_id: i64) -> std::path::PathBuf {
    env::temp_dir().join(format!("local-worker-chat-{turn_id}.txt"))
}

fn extract_text_from_json_lines(raw: &str) -> Option<String> {
    let mut values = Vec::new();
    for line in raw.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        collect_json_text(&value, &mut values);
    }
    let text = values.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn extract_codex_session_id(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(session_id) = find_uuid_field(&value, "session_id")
            .or_else(|| find_uuid_field(&value, "conversation_id"))
            .or_else(|| find_uuid_field(&value, "thread_id"))
        {
            return Some(session_id);
        }
    }
    None
}

fn find_uuid_field(value: &serde_json::Value, key: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(text)) = map.get(key) {
                if Uuid::parse_str(text).is_ok() {
                    return Some(text.clone());
                }
            }
            map.values().find_map(|value| find_uuid_field(value, key))
        }
        serde_json::Value::Array(items) => {
            items.iter().find_map(|value| find_uuid_field(value, key))
        }
        _ => None,
    }
}

fn collect_json_text(value: &serde_json::Value, values: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => {
            if text.len() > 1 && !looks_like_metadata(text) {
                values.push(text.clone());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_text(item, values);
            }
        }
        serde_json::Value::Object(map) => {
            for key in ["text", "content", "message", "summary", "result"] {
                if let Some(value) = map.get(key) {
                    collect_json_text(value, values);
                }
            }
        }
        _ => {}
    }
}

fn looks_like_metadata(text: &str) -> bool {
    matches!(
        text,
        "assistant" | "user" | "system" | "message" | "content" | "text" | "result"
    ) || text.starts_with("msg_")
        || text.starts_with("toolu_")
}

fn emit_log(app: &AppHandle, turn_id: i64, stream: &str, line: &str) {
    let _ = app.emit(
        "chat-log",
        ChatLogEvent {
            turn_id,
            stream: stream.to_string(),
            line: line.to_string(),
        },
    );
}

fn emit_turn_update(app: &AppHandle, turn_id: i64, session_id: i64, status: &str) {
    let _ = app.emit(
        "chat-turn-updated",
        ChatTurnUpdatedEvent {
            turn_id,
            session_id,
            status: status.to_string(),
        },
    );
}
