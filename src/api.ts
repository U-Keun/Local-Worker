import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  Project,
  QueuedTask,
  RunRecord,
  LogLine,
  SyncResult,
  CreateProjectRequest,
  WorkerHealth,
  ProjectSetupCheck,
  ProjectCreationPreview,
} from "./types";

function isTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

function requireTauri<T>(action: string): Promise<T> {
  return Promise.reject(new Error(`${action} is available in the Tauri desktop app.`));
}

function normalizeProjectName(name: string) {
  const normalized = name
    .trim()
    .split("")
    .map((char) => (/^[a-z0-9_.-]$/i.test(char) ? char : "-"))
    .join("")
    .replace(/^-+|-+$/g, "");
  return normalized || "local-worker-project";
}

function joinPath(parentPath: string, projectName: string) {
  const trimmed = parentPath.trim().replace(/\/+$/g, "");
  return trimmed ? `${trimmed}/${projectName}` : projectName;
}

const browserHealth: WorkerHealth = {
  polling_active: false,
  project_count: 0,
  running_count: 0,
  open_task_count: 0,
  failed_run_count: 0,
  last_sync_at: null,
  next_sync: null,
  codex_available: false,
  claude_available: false,
  gh_available: false,
  autostart_enabled: false,
  needs_attention: true,
  intervention_reason: "Desktop runtime is needed for live worker checks.",
  primary_action: "Complete setup",
};

function browserCreationPreview(request: CreateProjectRequest): ProjectCreationPreview {
  const projectName = normalizeProjectName(request.name);
  const checks: ProjectSetupCheck[] = [
    {
      id: "name",
      label: "Project name",
      status: request.name.trim() ? "passed" : "failed",
      detail: request.name.trim()
        ? "Name can be used for preview."
        : "Project name is required.",
    },
    {
      id: "parent",
      label: "Parent folder",
      status: request.parent_path.trim() ? "warning" : "failed",
      detail: request.parent_path.trim()
        ? "Folder availability is checked in the desktop app."
        : "Parent folder is required.",
    },
    {
      id: "github",
      label: "GitHub repo",
      status: request.create_github_repo ? "warning" : "passed",
      detail: request.create_github_repo
        ? "gh availability is checked in the desktop app."
        : "Local-only project.",
    },
  ];

  return {
    project_name: projectName,
    path: joinPath(request.parent_path, projectName),
    repo_name: request.create_github_repo ? projectName : null,
    checks,
    files: [
      "README.md",
      ".gitignore",
      "AGENTS.md",
      "tasks/queue.md",
      "tasks/done.md",
      "tasks/failed.md",
      "scripts/smoke-test.sh",
    ],
  };
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
  checkWorkerHealth: () =>
    isTauriRuntime()
      ? invoke<WorkerHealth>("check_worker_health")
      : Promise.resolve(browserHealth),
  checkProjectSetup: (path: string) =>
    isTauriRuntime()
      ? invoke<ProjectSetupCheck[]>("check_project_setup", { path })
      : Promise.resolve<ProjectSetupCheck[]>([
          {
            id: "desktop-runtime",
            label: "Desktop runtime",
            status: "warning",
            detail: "Project setup checks run in the Tauri desktop app.",
          },
        ]),
  previewProjectCreation: (request: CreateProjectRequest) =>
    isTauriRuntime()
      ? invoke<ProjectCreationPreview>("preview_project_creation", { request })
      : Promise.resolve(browserCreationPreview(request)),
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
