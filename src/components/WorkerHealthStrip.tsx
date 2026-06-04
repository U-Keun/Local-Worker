import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  Github,
  Laptop,
  Play,
  RefreshCw,
  Settings,
  Terminal,
  Zap,
} from "lucide-react";
import type { WorkerHealth } from "../types";

interface WorkerHealthStripProps {
  health: WorkerHealth | null;
  busy: boolean;
  onPrimaryAction: (action: WorkerHealth["primary_action"]) => void;
  onRefresh: () => void;
}

function formatDate(value: string | null) {
  if (!value) return "never";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function actionIcon(action: WorkerHealth["primary_action"]) {
  if (action === "Run next") return <Play size={17} />;
  if (action === "Review failure") return <AlertTriangle size={17} />;
  if (action === "Sync now") return <RefreshCw size={17} />;
  return <Settings size={17} />;
}

export function WorkerHealthStrip({
  health,
  busy,
  onPrimaryAction,
  onRefresh,
}: WorkerHealthStripProps) {
  const ready = Boolean(health && !health.needs_attention);
  const primaryAction = health?.primary_action ?? "Complete setup";

  return (
    <section className={`health-strip ${ready ? "ready" : "attention"}`}>
      <div className="health-title">
        {ready ? <CheckCircle2 size={22} /> : <AlertTriangle size={22} />}
        <div>
          <h3>Worker Health</h3>
          <p>{health?.intervention_reason ?? "Ready for GitHub issue work."}</p>
        </div>
      </div>

      <div className="health-items">
        <span className={`health-item ${health?.polling_active ? "ok" : "warn"}`}>
          <Zap size={15} />
          {health?.polling_active ? "Polling" : "Paused"}
        </span>
        <span className="health-item">
          <Clock3 size={15} />
          Last {formatDate(health?.last_sync_at ?? null)}
        </span>
        <span className="health-item">
          <Clock3 size={15} />
          Next {health?.next_sync ?? "manual"}
        </span>
        <span className={`health-item ${health?.gh_available ? "ok" : "warn"}`}>
          <Github size={15} />
          gh
        </span>
        <span className={`health-item ${health?.codex_available ? "ok" : "warn"}`}>
          <Terminal size={15} />
          codex
        </span>
        <span className={`health-item ${health?.claude_available ? "ok" : "warn"}`}>
          <Terminal size={15} />
          claude
        </span>
        <span className={`health-item ${health?.autostart_enabled ? "ok" : "warn"}`}>
          <Laptop size={15} />
          autostart
        </span>
      </div>

      <div className="health-actions">
        <button
          type="button"
          className="ghost-icon-button"
          disabled={busy}
          onClick={onRefresh}
          title="Refresh health"
        >
          <RefreshCw size={17} />
        </button>
        <button
          type="button"
          className="primary-health-action"
          disabled={busy}
          onClick={() => onPrimaryAction(primaryAction)}
        >
          {actionIcon(primaryAction)}
          {primaryAction}
        </button>
      </div>
    </section>
  );
}
