import {
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  ClipboardList,
  FileText,
  Plus,
  Wand2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import type {
  CreateLocalTaskResult,
  Project,
  RunRecord,
} from "../types";

interface TaskComposerPanelProps {
  project: Project | null;
  sourceRun: RunRecord | null;
  seedKey: number;
  open?: boolean;
  drawer?: boolean;
  disabled?: boolean;
  onClose?: () => void;
  onError: (message: string) => void;
  onCreated: (result: CreateLocalTaskResult) => Promise<void> | void;
}

interface TaskDraft {
  title: string;
  priority: "low" | "medium" | "high";
  goal: string;
  doneCriteria: string;
  constraints: string;
}

const defaultDoneCriteria = "Task satisfies the request.";
const defaultConstraints = "Keep the diff small.\nDo not modify unrelated files.";

function cleanTitle(value: string) {
  const title = value
    .split("\n")
    .map((line) => line.trim())
    .find(Boolean)
    ?.replace(/^#+\s*/, "")
    .replace(/^[-*]\s*/, "")
    .trim();
  if (!title) return "Local task";
  return title.length > 86 ? `${title.slice(0, 83).trim()}...` : title;
}

function splitLines(value: string) {
  return value
    .split("\n")
    .map((line) => line.trim().replace(/^[-*]\s*/, ""))
    .filter(Boolean);
}

function emptyDraft(): TaskDraft {
  return {
    title: "",
    priority: "medium",
    goal: "",
    doneCriteria: defaultDoneCriteria,
    constraints: defaultConstraints,
  };
}

function draftFromRequest(request: string): TaskDraft {
  return {
    title: cleanTitle(request),
    priority: "medium",
    goal: request.trim(),
    doneCriteria: defaultDoneCriteria,
    constraints: defaultConstraints,
  };
}

function draftFromRun(run: RunRecord): TaskDraft {
  const task = run.task_id ?? `run #${run.id}`;
  const summary = run.summary ?? "No run summary was recorded.";
  return {
    title: `Follow up ${task}`,
    priority: "high",
    goal: [
      "Create a focused follow-up task for the failed Local Worker run.",
      "",
      `Run ID: ${run.id}`,
      `Task: ${task}`,
      `Status: ${run.status}`,
      `Backend: ${run.backend}`,
      `Test command: ${run.test_command}`,
      `Summary: ${summary}`,
    ].join("\n"),
    doneCriteria: [
      "The failure is investigated and addressed.",
      "The configured test command passes.",
    ].join("\n"),
    constraints: defaultConstraints,
  };
}

export function TaskComposerPanel({
  project,
  sourceRun,
  seedKey,
  open = true,
  drawer = false,
  disabled = false,
  onClose,
  onError,
  onCreated,
}: TaskComposerPanelProps) {
  const [requestText, setRequestText] = useState("");
  const [draft, setDraft] = useState<TaskDraft>(emptyDraft);
  const [creating, setCreating] = useState(false);
  const [resultMessage, setResultMessage] = useState("");

  const panelClassName = [
    "task-composer-panel",
    "chat-panel",
    drawer ? "chat-drawer" : "",
    open ? "open" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const createDisabled = useMemo(
    () =>
      !project ||
      disabled ||
      creating ||
      !draft.title.trim() ||
      !draft.goal.trim(),
    [creating, disabled, draft.goal, draft.title, project],
  );

  function draftTask() {
    if (!requestText.trim()) {
      setDraft(emptyDraft());
      return;
    }
    setDraft(draftFromRequest(requestText));
    setResultMessage("Draft prepared. Review it before creating the task.");
  }

  async function createTask() {
    if (!project || createDisabled) return;
    setCreating(true);
    setResultMessage("");
    try {
      const result = await api.createLocalTask({
        project_id: project.id,
        source_run_id: sourceRun?.id ?? null,
        title: draft.title,
        priority: draft.priority,
        goal: draft.goal,
        done_criteria: splitLines(draft.doneCriteria),
        constraints: splitLines(draft.constraints),
      });
      setResultMessage(`${result.task.task_id} created. ${result.auto_run_status}`);
      await onCreated(result);
      setRequestText("");
      if (!sourceRun) {
        setDraft(emptyDraft());
      }
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    } finally {
      setCreating(false);
    }
  }

  useEffect(() => {
    setRequestText("");
    if (sourceRun) {
      setDraft(draftFromRun(sourceRun));
      setResultMessage("Follow-up draft prepared from the selected run.");
    } else {
      setDraft(emptyDraft());
      setResultMessage("");
    }
  }, [seedKey]);

  useEffect(() => {
    if (!open || !onClose) return;
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose?.();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  return (
    <section className={panelClassName} aria-hidden={drawer ? !open : undefined}>
      <div className="panel-heading chat-heading">
        <div>
          <h3>Task Composer</h3>
          <p>
            {project
              ? `${project.name} · creates queued tasks only`
              : "Select a project to compose a task."}
          </p>
        </div>
        <div className="chat-heading-actions">
          <ClipboardList size={16} />
          {onClose && (
            <button
              type="button"
              className="ghost-icon-button"
              onClick={onClose}
              title="Close composer"
            >
              <X size={16} />
            </button>
          )}
        </div>
      </div>

      {sourceRun && (
        <div className="composer-source-banner">
          <CircleAlert size={16} />
          <div>
            <strong>Create follow-up task</strong>
            <small>
              {sourceRun.task_id ?? `Run #${sourceRun.id}`} · {sourceRun.status}
            </small>
          </div>
        </div>
      )}

      <div className="composer-scroll">
        <div className="composer-note">
          <FileText size={16} />
          <span>
            Drafts do not run Codex or Claude. A task is added only after you
            create it.
          </span>
        </div>

        {!sourceRun && (
          <div className="composer-request">
            <label htmlFor="task-request">Request</label>
            <textarea
              id="task-request"
              value={requestText}
              onChange={(event) => setRequestText(event.target.value)}
              placeholder="Describe the work you want to turn into a task."
              disabled={!project || disabled || creating}
            />
            <button
              type="button"
              className="secondary-button"
              disabled={!project || disabled || creating || !requestText.trim()}
              onClick={draftTask}
              title="Draft task fields"
            >
              <Wand2 size={16} />
              Draft Task
            </button>
          </div>
        )}

        <div className="composer-form">
          <label>
            Title
            <input
              value={draft.title}
              onChange={(event) =>
                setDraft((current) => ({ ...current, title: event.target.value }))
              }
              placeholder="Short task title"
              disabled={!project || disabled || creating}
            />
          </label>

          <label>
            Priority
            <select
              value={draft.priority}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  priority: event.target.value as TaskDraft["priority"],
                }))
              }
              disabled={!project || disabled || creating}
            >
              <option value="low">low</option>
              <option value="medium">medium</option>
              <option value="high">high</option>
            </select>
          </label>

          <label>
            Goal
            <textarea
              value={draft.goal}
              onChange={(event) =>
                setDraft((current) => ({ ...current, goal: event.target.value }))
              }
              placeholder="What should be true after this task is done?"
              disabled={!project || disabled || creating}
            />
          </label>

          <label>
            Done criteria
            <textarea
              value={draft.doneCriteria}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  doneCriteria: event.target.value,
                }))
              }
              disabled={!project || disabled || creating}
            />
          </label>

          <label>
            Constraints
            <textarea
              value={draft.constraints}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  constraints: event.target.value,
                }))
              }
              disabled={!project || disabled || creating}
            />
          </label>
        </div>
      </div>

      <div className="composer-footer">
        <div className="composer-auto-run">
          {project?.auto_run ? <CheckCircle2 size={15} /> : <CircleAlert size={15} />}
          <span>
            {project?.auto_run
              ? "Auto run is on. The next open task can start after creation."
              : "Auto run is off. Created tasks stay queued."}
          </span>
        </div>
        {resultMessage && <p>{resultMessage}</p>}
        <button
          type="button"
          disabled={createDisabled}
          onClick={() => createTask().catch((error) => onError(String(error)))}
          title="Create queued task"
        >
          <Plus size={17} />
          Create Task
          <ChevronRight size={16} />
        </button>
      </div>
    </section>
  );
}
