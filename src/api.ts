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

export const api = {
  appStatus: () => invoke<AppStatus>("app_status"),
  listProjects: () => invoke<Project[]>("list_projects"),
  addProject: (path: string) => invoke<Project>("add_project", { path }),
  createProject: (request: CreateProjectRequest) =>
    invoke<Project>("create_project", { request }),
  updateProject: (project: Project) => invoke<Project>("update_project", { project }),
  listTasks: (projectId: number) => invoke<QueuedTask[]>("list_tasks", { projectId }),
  listRuns: (projectId: number) => invoke<RunRecord[]>("list_runs", { projectId }),
  listLogs: (runId: number) => invoke<LogLine[]>("list_logs", { runId }),
  syncGithubIssues: (projectId: number) =>
    invoke<SyncResult>("sync_github_issues", { projectId }),
  runNextTask: (projectId: number) => invoke<RunRecord>("run_next_task", { projectId }),
  startPolling: () => invoke<void>("start_polling"),
  stopPolling: () => invoke<void>("stop_polling"),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
};
