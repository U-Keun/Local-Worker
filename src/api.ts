import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  Project,
  QueuedTask,
  RunRecord,
  LogLine,
  SyncResult,
  CreateProjectRequest,
} from "./types";

function isTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

function requireTauri<T>(action: string): Promise<T> {
  return Promise.reject(new Error(`${action} is available in the Tauri desktop app.`));
}

export const api = {
  appStatus: () =>
    isTauriRuntime()
      ? invoke<AppStatus>("app_status")
      : Promise.resolve({
          codex_available: false,
          claude_available: false,
          gh_available: false,
          autostart_enabled: false,
        }),
  listProjects: () =>
    isTauriRuntime() ? invoke<Project[]>("list_projects") : Promise.resolve([]),
  addProject: (path: string) =>
    isTauriRuntime()
      ? invoke<Project>("add_project", { path })
      : requireTauri<Project>("Project registration"),
  createProject: (request: CreateProjectRequest) =>
    isTauriRuntime()
      ? invoke<Project>("create_project", { request })
      : requireTauri<Project>("Project creation"),
  updateProject: (project: Project) =>
    isTauriRuntime()
      ? invoke<Project>("update_project", { project })
      : requireTauri<Project>("Project updates"),
  listTasks: (projectId: number) =>
    isTauriRuntime()
      ? invoke<QueuedTask[]>("list_tasks", { projectId })
      : Promise.resolve([]),
  listRuns: (projectId: number) =>
    isTauriRuntime()
      ? invoke<RunRecord[]>("list_runs", { projectId })
      : Promise.resolve([]),
  listLogs: (runId: number) =>
    isTauriRuntime()
      ? invoke<LogLine[]>("list_logs", { runId })
      : Promise.resolve([]),
  syncGithubIssues: (projectId: number) =>
    isTauriRuntime()
      ? invoke<SyncResult>("sync_github_issues", { projectId })
      : requireTauri<SyncResult>("GitHub issue sync"),
  runNextTask: (projectId: number) =>
    isTauriRuntime()
      ? invoke<RunRecord>("run_next_task", { projectId })
      : requireTauri<RunRecord>("Task runs"),
  startPolling: () =>
    isTauriRuntime() ? invoke<void>("start_polling") : Promise.resolve(),
  stopPolling: () =>
    isTauriRuntime() ? invoke<void>("stop_polling") : Promise.resolve(),
  setAutostart: (enabled: boolean) =>
    isTauriRuntime()
      ? invoke<boolean>("set_autostart", { enabled })
      : requireTauri<boolean>("Autostart"),
};
