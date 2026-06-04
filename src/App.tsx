import { listen } from "@tauri-apps/api/event";
import {
  Activity,
  CheckCircle2,
  CircleAlert,
  Clock3,
  FolderPlus,
  Github,
  Laptop,
  Play,
  Plus,
  RefreshCw,
  Settings,
  Square,
  Terminal,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "./api";
import type {
  AppStatus,
  Project,
  QueuedTask,
  RunLogEvent,
  RunRecord,
  RunUpdatedEvent,
} from "./types";

const statusClass: Record<string, string> = {
  open: "status-open",
  "in-progress": "status-running",
  running: "status-running",
  done: "status-done",
  passed: "status-done",
  failed: "status-failed",
  blocked: "status-failed",
};

function classForStatus(status: string) {
  return statusClass[status] ?? "status-muted";
}

function formatDate(value: string | null) {
  if (!value) return "not finished";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function emptyProject(): Project | null {
  return null;
}

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null);
  const [tasks, setTasks] = useState<QueuedTask[]>([]);
  const [runs, setRuns] = useState<RunRecord[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [newPath, setNewPath] = useState("");
  const [createName, setCreateName] = useState("");
  const [createParentPath, setCreateParentPath] = useState("");
  const [createGithubRepo, setCreateGithubRepo] = useState(false);
  const [createPrivateRepo, setCreatePrivateRepo] = useState(true);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);

  const selectedProject = useMemo(
    () => projects.find((project) => project.id === selectedProjectId) ?? emptyProject(),
    [projects, selectedProjectId],
  );

  const selectedRun = useMemo(
    () => runs.find((run) => run.id === selectedRunId) ?? null,
    [runs, selectedRunId],
  );

  async function refresh(projectId = selectedProjectId) {
    const [nextStatus, nextProjects] = await Promise.all([
      api.appStatus(),
      api.listProjects(),
    ]);
    setStatus(nextStatus);
    setProjects(nextProjects);

    const nextProjectId =
      projectId ?? selectedProjectId ?? nextProjects[0]?.id ?? null;
    setSelectedProjectId(nextProjectId);

    if (nextProjectId) {
      const [nextTasks, nextRuns] = await Promise.all([
        api.listTasks(nextProjectId),
        api.listRuns(nextProjectId),
      ]);
      setTasks(nextTasks);
      setRuns(nextRuns);
      setSelectedRunId((current) => current ?? nextRuns[0]?.id ?? null);
    } else {
      setTasks([]);
      setRuns([]);
      setSelectedRunId(null);
      setLogs([]);
    }
  }

  async function refreshLogs(runId = selectedRunId) {
    if (!runId) {
      setLogs([]);
      return;
    }
    const nextLogs = await api.listLogs(runId);
    setLogs(nextLogs.map((log) => `[${log.stream}] ${log.line}`));
  }

  async function withBusy(action: () => Promise<void>, success: string) {
    setBusy(true);
    setMessage("");
    try {
      await action();
      setMessage(success);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    refresh().catch((error) => setMessage(String(error)));
    api.startPolling().catch((error) => setMessage(String(error)));

    const unlistenLog = listen<RunLogEvent>("run-log", (event) => {
      if (event.payload.run_id === selectedRunId) {
        setLogs((current) => [
          ...current.slice(-500),
          `[${event.payload.stream}] ${event.payload.line}`,
        ]);
      }
    });
    const unlistenRun = listen<RunUpdatedEvent>("run-updated", () => {
      refresh().catch((error) => setMessage(String(error)));
    });

    return () => {
      unlistenLog.then((fn) => fn());
      unlistenRun.then((fn) => fn());
    };
  }, [selectedRunId]);

  useEffect(() => {
    refreshLogs().catch((error) => setMessage(String(error)));
  }, [selectedRunId]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      refresh().catch((error) => setMessage(String(error)));
    }, 5000);
    return () => window.clearInterval(timer);
  }, [selectedProjectId]);

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <Laptop size={22} />
          <div>
            <h1>Local Worker</h1>
            <p>Home Mac development runner</p>
          </div>
        </div>

        <div className="project-list">
          {projects.map((project) => (
            <button
              key={project.id}
              className={`project-button ${project.id === selectedProjectId ? "selected" : ""}`}
              onClick={() => {
                setSelectedProjectId(project.id);
                refresh(project.id).catch((error) => setMessage(String(error)));
              }}
            >
              <span>{project.name}</span>
              <small>{project.repo ?? project.path}</small>
            </button>
          ))}
        </div>

        <div className="cli-status">
          <span className={status?.gh_available ? "ok" : "missing"}>gh</span>
          <span className={status?.codex_available ? "ok" : "missing"}>codex</span>
          <span className={status?.claude_available ? "ok" : "missing"}>claude</span>
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <h2>{selectedProject?.name ?? "No project selected"}</h2>
            <p>{selectedProject?.path ?? "Register a repository to begin polling GitHub issues."}</p>
          </div>
          <div className="actions">
            <button
              disabled={!selectedProject || busy}
              onClick={() =>
                selectedProject &&
                withBusy(
                  async () => {
                    const result = await api.syncGithubIssues(selectedProject.id);
                    await refresh(selectedProject.id);
                    setMessage(
                      `Sync complete: ${result.added} added, ${result.skipped} skipped, ${result.issues_seen} seen`,
                    );
                  },
                  "Issue sync complete",
                )
              }
              title="Sync GitHub issues"
            >
              <Github size={18} />
              Sync
            </button>
            <button
              disabled={!selectedProject || busy}
              onClick={() =>
                selectedProject &&
                withBusy(
                  async () => {
                    const run = await api.runNextTask(selectedProject.id);
                    setSelectedRunId(run.id);
                    await refresh(selectedProject.id);
                  },
                  "Run started",
                )
              }
              title="Run next task"
            >
              <Play size={18} />
              Run
            </button>
            <button
              disabled={busy}
              onClick={() => refresh().catch((error) => setMessage(String(error)))}
              title="Refresh"
            >
              <RefreshCw size={18} />
            </button>
          </div>
        </header>

        {message && <div className="message">{message}</div>}

        <section className="setup-grid">
          <form
            className="setup-panel"
            onSubmit={(event) => {
              event.preventDefault();
              if (!newPath.trim()) return;
              withBusy(async () => {
                const project = await api.addProject(newPath.trim());
                setNewPath("");
                await refresh(project.id);
              }, "Project added").catch(() => undefined);
            }}
          >
            <div className="setup-heading">
              <Plus size={18} />
              <h3>Register Existing Repo</h3>
            </div>
            <div className="inline-form">
              <input
                value={newPath}
                onChange={(event) => setNewPath(event.target.value)}
                placeholder="/path/to/repo"
              />
              <button type="submit" disabled={busy} title="Add project">
                <Plus size={18} />
              </button>
            </div>
          </form>

          <form
            className="setup-panel"
            onSubmit={(event) => {
              event.preventDefault();
              if (!createName.trim() || !createParentPath.trim()) return;
              withBusy(async () => {
                const project = await api.createProject({
                  name: createName.trim(),
                  parent_path: createParentPath.trim(),
                  create_github_repo: createGithubRepo,
                  private_repo: createPrivateRepo,
                });
                setCreateName("");
                await refresh(project.id);
              }, "Project created").catch(() => undefined);
            }}
          >
            <div className="setup-heading">
              <FolderPlus size={18} />
              <h3>Create New Project</h3>
            </div>
            <div className="create-form">
              <input
                value={createName}
                onChange={(event) => setCreateName(event.target.value)}
                placeholder="project-name"
              />
              <input
                value={createParentPath}
                onChange={(event) => setCreateParentPath(event.target.value)}
                placeholder="/parent/folder"
              />
              <label>
                <input
                  type="checkbox"
                  checked={createGithubRepo}
                  onChange={(event) => setCreateGithubRepo(event.target.checked)}
                />
                Create GitHub repo
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={createPrivateRepo}
                  disabled={!createGithubRepo}
                  onChange={(event) => setCreatePrivateRepo(event.target.checked)}
                />
                Private
              </label>
              <button type="submit" disabled={busy} title="Create project">
                <FolderPlus size={18} />
                Create
              </button>
            </div>
          </form>
        </section>

        <section className="summary-grid">
          <div className="metric">
            <Clock3 size={18} />
            <span>{tasks.filter((task) => task.status === "open").length}</span>
            <small>open tasks</small>
          </div>
          <div className="metric">
            <Activity size={18} />
            <span>{runs.filter((run) => run.status === "running").length}</span>
            <small>running</small>
          </div>
          <div className="metric">
            <CheckCircle2 size={18} />
            <span>{runs.filter((run) => run.status === "passed").length}</span>
            <small>passed runs</small>
          </div>
          <div className="metric">
            <CircleAlert size={18} />
            <span>{runs.filter((run) => run.status === "failed").length}</span>
            <small>failed runs</small>
          </div>
        </section>

        <section className="content-grid">
          <div className="panel">
            <div className="panel-heading">
              <h3>Task Queue</h3>
              <Square size={16} />
            </div>
            <div className="task-list">
              {tasks.map((task) => (
                <article key={task.task_id} className="task-row">
                  <div>
                    <strong>{task.task_id}: {task.title}</strong>
                    <p>{task.goal}</p>
                  </div>
                  <span className={`pill ${classForStatus(task.status)}`}>{task.status}</span>
                </article>
              ))}
              {tasks.length === 0 && <p className="empty">No queued tasks found.</p>}
            </div>
          </div>

          <div className="panel">
            <div className="panel-heading">
              <h3>Runs</h3>
              <Terminal size={16} />
            </div>
            <div className="run-list">
              {runs.map((run) => (
                <button
                  key={run.id}
                  className={`run-row ${run.id === selectedRunId ? "selected" : ""}`}
                  onClick={() => setSelectedRunId(run.id)}
                >
                  <span>{run.task_id ?? "manual"} · {run.backend}</span>
                  <small>{formatDate(run.started_at)}</small>
                  <span className={`pill ${classForStatus(run.status)}`}>{run.status}</span>
                </button>
              ))}
              {runs.length === 0 && <p className="empty">No runs yet.</p>}
            </div>
          </div>
        </section>

        <section className="log-panel">
          <div className="panel-heading">
            <div>
              <h3>Execution Log</h3>
              <p>
                {selectedRun
                  ? `${selectedRun.task_id ?? "manual"} · ${selectedRun.branch ?? "no branch"} · ${formatDate(selectedRun.finished_at)}`
                  : "Select a run to inspect output."}
              </p>
            </div>
            <Terminal size={16} />
          </div>
          <pre>{logs.join("\n")}</pre>
        </section>

        <section className="settings-strip">
          <Settings size={18} />
          <label>
            <input
              type="checkbox"
              checked={Boolean(status?.autostart_enabled)}
              onChange={(event) =>
                withBusy(async () => {
                  const enabled = await api.setAutostart(event.target.checked);
                  setStatus((current) =>
                    current ? { ...current, autostart_enabled: enabled } : current,
                  );
                }, "Autostart updated")
              }
            />
            Start Local Worker when this Mac logs in
          </label>
        </section>
      </section>
    </main>
  );
}
