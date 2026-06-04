import { listen } from "@tauri-apps/api/event";
import {
  Bot,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  MessageSquarePlus,
  PlayCircle,
  Send,
  Sparkles,
  Terminal,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, isTauriRuntime } from "../api";
import type {
  AgentBackend,
  AppStatus,
  ChatLogEvent,
  ChatMessage,
  ChatMessageEvent,
  ChatSession,
  ChatTurn,
  ChatTurnUpdatedEvent,
  Project,
  RunRecord,
} from "../types";

interface AgentChatPanelProps {
  project: Project | null;
  selectedRun: RunRecord | null;
  status: AppStatus | null;
  open?: boolean;
  drawer?: boolean;
  disabled?: boolean;
  onClose?: () => void;
  onError: (message: string) => void;
  onActiveTurnChange?: (turn: ChatTurn | null) => void;
}

function formatTime(value: string | null) {
  if (!value) return "";
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function backendAvailable(status: AppStatus | null, backend: AgentBackend) {
  return backend === "codex"
    ? Boolean(status?.codex_available)
    : Boolean(status?.claude_available);
}

function turnLabel(turn: ChatTurn | null) {
  if (!turn) return "idle";
  if (turn.status === "running") return "responding";
  if (turn.status === "passed") return "tests passed";
  if (turn.status === "failed") return "needs review";
  return turn.status;
}

function compactTitle(session: ChatSession) {
  const prefix = session.run_id ? `Run #${session.run_id}` : "Project";
  return `${prefix} · ${session.backend}`;
}

export function AgentChatPanel({
  project,
  selectedRun,
  status,
  open = true,
  drawer = false,
  disabled = false,
  onClose,
  onError,
  onActiveTurnChange,
}: AgentChatPanelProps) {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<number | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [turns, setTurns] = useState<ChatTurn[]>([]);
  const [liveLogs, setLiveLogs] = useState<string[]>([]);
  const [backend, setBackend] = useState<AgentBackend>("codex");
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [sending, setSending] = useState(false);
  const turnIdsRef = useRef<Set<number>>(new Set());

  const selectedSession = useMemo(
    () => sessions.find((session) => session.id === selectedSessionId) ?? null,
    [sessions, selectedSessionId],
  );
  const latestTurn = turns[0] ?? null;
  const activeTurn = turns.find((turn) => turn.status === "running") ?? null;
  const canUseBackend = backendAvailable(status, backend);
  const runCanDiscuss =
    selectedRun?.status === "failed" || selectedRun?.status === "blocked";
  const panelClassName = [
    "chat-panel",
    drawer ? "chat-drawer" : "",
    open ? "open" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const loadSessions = useCallback(
    async (preferredSessionId?: number | null) => {
      if (!project) {
        setSessions([]);
        setSelectedSessionId(null);
        return;
      }
      const nextSessions = await api.listChatSessions(project.id);
      setSessions(nextSessions);
      setSelectedSessionId((current) => {
        const candidate = preferredSessionId ?? current;
        if (candidate && nextSessions.some((session) => session.id === candidate)) {
          return candidate;
        }
        return nextSessions[0]?.id ?? null;
      });
    },
    [project],
  );

  const loadConversation = useCallback(async (sessionId: number | null) => {
    if (!sessionId) {
      setMessages([]);
      setTurns([]);
      turnIdsRef.current = new Set();
      return;
    }
    const [nextMessages, nextTurns] = await Promise.all([
      api.listChatMessages(sessionId),
      api.listChatTurns(sessionId),
    ]);
    setMessages(nextMessages);
    setTurns(nextTurns);
    turnIdsRef.current = new Set(nextTurns.map((turn) => turn.id));
  }, []);

  async function createSession(run: RunRecord | null = null) {
    if (!project) return null;
    const nextSession = await api.createChatSession({
      project_id: project.id,
      run_id: run?.id ?? null,
      backend,
      title: run ? `${run.task_id ?? "Manual run"} discussion` : "Project chat",
    });
    await loadSessions(nextSession.id);
    await loadConversation(nextSession.id);
    setLiveLogs([]);
    return nextSession;
  }

  async function sendMessage() {
    const content = input.trim();
    if (!project || !content || sending || disabled) return;
    if (!canUseBackend) {
      onError(`${backend} CLI is not available on this Mac.`);
      return;
    }

    setSending(true);
    try {
      const session = selectedSession ?? (await createSession(null));
      if (!session) return;
      setInput("");
      setLiveLogs([]);
      const turn = await api.sendChatMessage({ session_id: session.id, content });
      setTurns((current) => [turn, ...current]);
      turnIdsRef.current = new Set([...turnIdsRef.current, turn.id]);
      await loadConversation(session.id);
      await loadSessions(session.id);
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    } finally {
      setSending(false);
    }
  }

  useEffect(() => {
    if (!project) {
      setBackend("codex");
      return;
    }
    setBackend(project.agent_backend);
    loadSessions().catch((error) =>
      onError(error instanceof Error ? error.message : String(error)),
    );
  }, [project?.id, project?.agent_backend, loadSessions, onError]);

  useEffect(() => {
    loadConversation(selectedSessionId).catch((error) =>
      onError(error instanceof Error ? error.message : String(error)),
    );
  }, [selectedSessionId, loadConversation, onError]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const unlistenMessage = listen<ChatMessageEvent>("chat-message", (event) => {
      if (event.payload.session_id !== selectedSessionId) return;
      setMessages((current) => {
        if (current.some((message) => message.id === event.payload.message.id)) {
          return current;
        }
        return [...current, event.payload.message];
      });
    });
    const unlistenTurn = listen<ChatTurnUpdatedEvent>("chat-turn-updated", (event) => {
      if (event.payload.session_id !== selectedSessionId) return;
      loadConversation(selectedSessionId).catch((error) =>
        onError(error instanceof Error ? error.message : String(error)),
      );
    });
    const unlistenLog = listen<ChatLogEvent>("chat-log", (event) => {
      if (!turnIdsRef.current.has(event.payload.turn_id)) return;
      setLiveLogs((current) => [
        ...current.slice(-240),
        `[${event.payload.stream}] ${event.payload.line}`,
      ]);
    });

    return () => {
      unlistenMessage.then((fn) => fn());
      unlistenTurn.then((fn) => fn());
      unlistenLog.then((fn) => fn());
    };
  }, [selectedSessionId, loadConversation, onError]);

  useEffect(() => {
    onActiveTurnChange?.(activeTurn ?? null);
  }, [activeTurn, onActiveTurnChange]);

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
          <h3>Agent Chat</h3>
          <p>
            {project
              ? `${project.name} · ${project.branch_prefix} branch · tests after each reply`
              : "Select a project to chat with Codex or Claude."}
          </p>
        </div>
        <div className="chat-heading-actions">
          <Bot size={16} />
          {onClose && (
            <button
              type="button"
              className="ghost-icon-button"
              onClick={onClose}
              title="Close chat"
            >
              <X size={16} />
            </button>
          )}
        </div>
      </div>

      <div className="chat-toolbar">
        <div className="segmented-control compact">
          {(["codex", "claude"] as AgentBackend[]).map((item) => (
            <button
              key={item}
              type="button"
              className={backend === item ? "selected" : ""}
              disabled={!project || sending || activeTurn !== null}
              onClick={() => setBackend(item)}
              title={`${item} backend`}
            >
              {item}
            </button>
          ))}
        </div>
        <span className={`backend-state ${canUseBackend ? "ok" : "missing"}`}>
          {canUseBackend ? "available" : "missing"}
        </span>
        <button
          type="button"
          className="secondary-button"
          disabled={!project || loading || sending || disabled}
          onClick={() => {
            setLoading(true);
            createSession(null)
              .catch((error) =>
                onError(error instanceof Error ? error.message : String(error)),
              )
              .finally(() => setLoading(false));
          }}
          title="Start project chat"
        >
          <MessageSquarePlus size={16} />
          New Chat
        </button>
      </div>

      {runCanDiscuss && (
        <button
          type="button"
          className="run-discuss-button"
          disabled={!project || loading || sending || disabled}
          onClick={() => {
            setLoading(true);
            createSession(selectedRun)
              .catch((error) =>
                onError(error instanceof Error ? error.message : String(error)),
              )
              .finally(() => setLoading(false));
          }}
          title="Discuss selected failed run with agent"
        >
          <CircleAlert size={16} />
          Discuss selected failure
          <ChevronRight size={16} />
        </button>
      )}

      <div className="chat-session-list">
        {sessions.map((session) => (
          <button
            key={session.id}
            type="button"
            className={`chat-session-button ${
              session.id === selectedSessionId ? "selected" : ""
            }`}
            onClick={() => {
              setSelectedSessionId(session.id);
              setLiveLogs([]);
            }}
          >
            <span>{session.title}</span>
            <small>{compactTitle(session)} · {formatTime(session.updated_at)}</small>
          </button>
        ))}
        {sessions.length === 0 && (
          <p className="empty chat-empty">No chat sessions yet.</p>
        )}
      </div>

      <div className="chat-status-line">
        <span className={`pill ${activeTurn ? "status-running" : "status-muted"}`}>
          {turnLabel(activeTurn ?? latestTurn)}
        </span>
        {latestTurn?.test_status === 0 && <CheckCircle2 size={15} />}
        {latestTurn && latestTurn.test_status !== null && latestTurn.test_status !== 0 && (
          <CircleAlert size={15} />
        )}
        {latestTurn?.test_command && <small>{latestTurn.test_command}</small>}
      </div>

      <div className="chat-timeline">
        {messages.map((message) => (
          <article key={message.id} className={`chat-message ${message.role}`}>
            <div className="chat-message-meta">
              <span>{message.role}</span>
              <small>{formatTime(message.created_at)}</small>
            </div>
            <p>{message.content}</p>
          </article>
        ))}
        {messages.length === 0 && (
          <div className="chat-placeholder">
            <Sparkles size={18} />
            <span>Ask for implementation help, failure analysis, or a small follow-up edit.</span>
          </div>
        )}
      </div>

      {liveLogs.length > 0 && (
        <div className="chat-live-log">
          <div>
            <Terminal size={15} />
            <span>Live output</span>
          </div>
          <pre>{liveLogs.join("\n")}</pre>
        </div>
      )}

      <form
        className="chat-composer"
        onSubmit={(event) => {
          event.preventDefault();
          sendMessage().catch((error) =>
            onError(error instanceof Error ? error.message : String(error)),
          );
        }}
      >
        <textarea
          value={input}
          disabled={!project || sending || Boolean(activeTurn) || disabled}
          onChange={(event) => setInput(event.target.value)}
          placeholder="Ask the agent to inspect, explain, or make a focused edit..."
        />
        <button
          type="submit"
          disabled={
            !project ||
            !input.trim() ||
            sending ||
            Boolean(activeTurn) ||
            !canUseBackend ||
            disabled
          }
          title="Send message"
        >
          {sending || activeTurn ? <PlayCircle size={17} /> : <Send size={17} />}
        </button>
      </form>
    </section>
  );
}
