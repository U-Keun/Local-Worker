import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  FolderPlus,
  Github,
  Lock,
  Terminal,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import type {
  AgentBackend,
  CreateProjectRequest,
  Project,
  ProjectCreationPreview,
  ProjectSetupCheck,
} from "../types";

interface SetupWizardProps {
  open: boolean;
  onClose: () => void;
  onCreated: (project: Project) => Promise<void> | void;
}

const steps = ["Name", "GitHub", "Automation", "Review"];

const defaultRequest: CreateProjectRequest = {
  name: "",
  parent_path: "",
  create_github_repo: false,
  private_repo: true,
  agent_backend: "codex",
  test_command: "bash scripts/smoke-test.sh",
  issue_label: "agent-task",
  branch_prefix: "codex",
};

function statusIcon(check: ProjectSetupCheck) {
  if (check.status === "passed") return <CheckCircle2 size={16} />;
  return <AlertTriangle size={16} />;
}

export function SetupWizard({ open, onClose, onCreated }: SetupWizardProps) {
  const [step, setStep] = useState(0);
  const [request, setRequest] = useState<CreateProjectRequest>(defaultRequest);
  const [preview, setPreview] = useState<ProjectCreationPreview | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const currentRequest = useMemo(
    () => ({
      ...request,
      name: request.name.trim(),
      parent_path: request.parent_path.trim(),
      test_command: request.test_command?.trim() || defaultRequest.test_command,
      issue_label: request.issue_label?.trim() || defaultRequest.issue_label,
      branch_prefix: request.branch_prefix?.trim() || defaultRequest.branch_prefix,
    }),
    [request],
  );

  const hasFailedPreview = preview?.checks.some((check) => check.status === "failed") ?? true;
  const canContinue =
    step === 0
      ? Boolean(currentRequest.name && currentRequest.parent_path)
      : step === 2
        ? Boolean(currentRequest.agent_backend && currentRequest.test_command)
        : step === 3
          ? Boolean(preview && !hasFailedPreview)
          : true;

  useEffect(() => {
    if (!open) return;
    setError("");
  }, [open]);

  useEffect(() => {
    if (!open || step !== 3) return;
    let cancelled = false;
    setPreview(null);
    api
      .previewProjectCreation(currentRequest)
      .then((nextPreview) => {
        if (!cancelled) setPreview(nextPreview);
      })
      .catch((nextError) => {
        if (!cancelled) {
          setError(nextError instanceof Error ? nextError.message : String(nextError));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [currentRequest, open, step]);

  if (!open) return null;

  function updateRequest(update: Partial<CreateProjectRequest>) {
    setRequest((current) => ({ ...current, ...update }));
  }

  function closeAndReset() {
    setStep(0);
    setPreview(null);
    setError("");
    setAdvancedOpen(false);
    setRequest(defaultRequest);
    onClose();
  }

  async function createProject() {
    setBusy(true);
    setError("");
    try {
      const project = await api.createProject(currentRequest);
      await onCreated(project);
      closeAndReset();
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : String(nextError));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="wizard-backdrop" role="presentation">
      <section className="wizard-sheet" role="dialog" aria-modal="true" aria-label="Create project">
        <header className="wizard-header">
          <div>
            <h3>Create New Project</h3>
            <p>{preview?.path ?? "Worker-ready local repository"}</p>
          </div>
          <button type="button" className="ghost-icon-button" onClick={closeAndReset} title="Close">
            <X size={18} />
          </button>
        </header>

        <nav className="wizard-steps" aria-label="Project setup steps">
          {steps.map((label, index) => (
            <button
              type="button"
              key={label}
              className={`wizard-step ${index === step ? "active" : ""} ${index < step ? "done" : ""}`}
              onClick={() => setStep(index)}
            >
              <span>{index + 1}</span>
              {label}
            </button>
          ))}
        </nav>

        <div className="wizard-body">
          {step === 0 && (
            <div className="wizard-fields">
              <label>
                Project name
                <input
                  value={request.name}
                  onChange={(event) => updateRequest({ name: event.target.value })}
                  placeholder="my-worker-project"
                />
              </label>
              <label>
                Parent folder
                <input
                  value={request.parent_path}
                  onChange={(event) => updateRequest({ parent_path: event.target.value })}
                  placeholder="/Users/me/Projects"
                />
              </label>
            </div>
          )}

          {step === 1 && (
            <div className="wizard-fields">
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={request.create_github_repo}
                  onChange={(event) =>
                    updateRequest({ create_github_repo: event.target.checked })
                  }
                />
                Create GitHub repository
              </label>
              <div className="segmented-control" aria-label="Repository visibility">
                <button
                  type="button"
                  className={request.private_repo ? "selected" : ""}
                  disabled={!request.create_github_repo}
                  onClick={() => updateRequest({ private_repo: true })}
                >
                  <Lock size={16} />
                  Private
                </button>
                <button
                  type="button"
                  className={!request.private_repo ? "selected" : ""}
                  disabled={!request.create_github_repo}
                  onClick={() => updateRequest({ private_repo: false })}
                >
                  <Github size={16} />
                  Public
                </button>
              </div>
            </div>
          )}

          {step === 2 && (
            <div className="wizard-fields">
              <div className="segmented-control" aria-label="Agent backend">
                {(["codex", "claude"] as AgentBackend[]).map((backend) => (
                  <button
                    type="button"
                    key={backend}
                    className={request.agent_backend === backend ? "selected" : ""}
                    onClick={() => updateRequest({ agent_backend: backend })}
                  >
                    <Terminal size={16} />
                    {backend}
                  </button>
                ))}
              </div>
              <label>
                Test command
                <input
                  value={request.test_command ?? ""}
                  onChange={(event) => updateRequest({ test_command: event.target.value })}
                  placeholder="bash scripts/smoke-test.sh"
                />
              </label>
              <button
                type="button"
                className="advanced-toggle"
                onClick={() => setAdvancedOpen((current) => !current)}
              >
                <ChevronDown size={16} className={advancedOpen ? "open" : ""} />
                Advanced settings
              </button>
              {advancedOpen && (
                <div className="advanced-fields">
                  <label>
                    Issue label
                      <input
                      value={request.issue_label ?? ""}
                      onChange={(event) => updateRequest({ issue_label: event.target.value })}
                      placeholder="agent-task"
                    />
                  </label>
                  <label>
                    Branch prefix
                    <input
                      value={request.branch_prefix ?? ""}
                      onChange={(event) => updateRequest({ branch_prefix: event.target.value })}
                      placeholder="codex"
                    />
                  </label>
                </div>
              )}
            </div>
          )}

          {step === 3 && (
            <div className="wizard-review">
              <div className="preview-summary">
                <FolderPlus size={20} />
                <div>
                  <strong>{preview?.project_name ?? currentRequest.name}</strong>
                  <span>{preview?.path ?? "Checking project path..."}</span>
                </div>
              </div>
              <div className="check-list">
                {preview?.checks.map((check) => (
                  <div key={check.id} className={`check-row ${check.status}`}>
                    {statusIcon(check)}
                    <div>
                      <strong>{check.label}</strong>
                      <span>{check.detail}</span>
                    </div>
                  </div>
                ))}
              </div>
              <div className="file-list">
                {preview?.files.map((file) => <span key={file}>{file}</span>)}
              </div>
            </div>
          )}
        </div>

        {error && <div className="wizard-error">{error}</div>}

        <footer className="wizard-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={busy || step === 0}
            onClick={() => setStep((current) => Math.max(0, current - 1))}
          >
            <ChevronLeft size={17} />
            Back
          </button>
          {step < steps.length - 1 ? (
            <button
              type="button"
              className="primary-button"
              disabled={busy || !canContinue}
              onClick={() => setStep((current) => Math.min(steps.length - 1, current + 1))}
            >
              Next
              <ChevronRight size={17} />
            </button>
          ) : (
            <button
              type="button"
              className="primary-button"
              disabled={busy || !canContinue}
              onClick={createProject}
            >
              <FolderPlus size={17} />
              Create
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}
