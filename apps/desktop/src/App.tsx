import {
  CalendarDays,
  Check,
  CirclePlus,
  ListTodo,
  RefreshCw,
  Server,
} from "lucide-react";
import {
  FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import {
  PlanningRequestError,
  completeTask,
  createScheduleEntry,
  createTask,
  exchangePairingCode,
  fetchPlanning,
  refreshDeviceSession,
  type ScheduleEntry,
  type SessionTokens,
  type Task,
} from "./api/planning";
import {
  AgentRequestError,
  createConversation,
  fetchAgentJob,
  fetchConversationMessages,
  fetchConversations,
  fetchLatestConversationJob,
  queueAgentTurn,
  type AgentJob,
  type Conversation,
  type ConversationMessage,
} from "./api/agent";
import { ConversationWorkspace } from "./components/ConversationWorkspace";
import { copy } from "./copy";
import {
  clearDeviceSession,
  readDeviceSession,
  readOrCreateInstallationId,
  saveDeviceSession,
} from "./device-session";
import { createUuidV7 } from "./uuid";

const defaultApiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? "";

type AppMode = "setup" | "loading" | "ready" | "error";
type AppView = "today" | "conversations";
type ConversationJobs = Record<string, AgentJob>;

export default function App() {
  const [apiBaseUrl, setApiBaseUrl] = useState(defaultApiBaseUrl);
  const [tokens, setTokens] = useState<SessionTokens | undefined>(undefined);
  const [sessionLoaded, setSessionLoaded] = useState(false);
  const [mode, setMode] = useState<AppMode>("loading");
  const [schedule, setSchedule] = useState<ScheduleEntry[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [view, setView] = useState<AppView>("today");
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [selectedConversationId, setSelectedConversationId] = useState<
    string | undefined
  >(undefined);
  const [conversationMessages, setConversationMessages] = useState<
    ConversationMessage[]
  >([]);
  const [conversationLoading, setConversationLoading] = useState(false);
  const [conversationError, setConversationError] = useState<
    string | undefined
  >(undefined);
  const [conversationJobs, setConversationJobs] = useState<ConversationJobs>(
    {},
  );
  const pendingConversationId = useRef<string | undefined>(undefined);
  const [message, setMessage] = useState<string | undefined>(undefined);

  const refreshConversations = useCallback(async () => {
    if (!tokens) return;
    setConversationLoading(true);
    setConversationError(undefined);
    try {
      setConversations(
        await fetchConversations(apiBaseUrl, tokens.accessToken),
      );
    } catch {
      setConversationError(copy.messages.conversationLoadNotice);
    } finally {
      setConversationLoading(false);
    }
  }, [apiBaseUrl, tokens]);

  const loadConversationMessages = useCallback(
    async (conversationId: string) => {
      if (!tokens) return;
      setConversationLoading(true);
      setConversationError(undefined);
      try {
        setConversationMessages(
          await fetchConversationMessages(
            apiBaseUrl,
            tokens.accessToken,
            conversationId,
          ),
        );
      } catch (error) {
        setConversationMessages([]);
        setConversationError(
          error instanceof AgentRequestError && error.code === "notFound"
            ? copy.messages.conversationChanged
            : copy.messages.conversationLoadNotice,
        );
      } finally {
        setConversationLoading(false);
      }
    },
    [apiBaseUrl, tokens],
  );

  const refresh = useCallback(async () => {
    if (!sessionLoaded) return;
    if (!tokens) return;
    setMode("loading");
    setMessage(undefined);
    try {
      const now = new Date();
      const week = new Date(now);
      week.setDate(now.getDate() + 7);
      const data = await fetchPlanning(
        apiBaseUrl,
        tokens.accessToken,
        now,
        week,
      );
      setSchedule(data.schedule);
      setTasks(data.tasks);
      setMode("ready");
      void refreshConversations();
    } catch (error) {
      if (
        error instanceof PlanningRequestError &&
        error.code === "unauthorized"
      ) {
        try {
          const refreshed = await refreshDeviceSession(
            apiBaseUrl,
            tokens.refreshToken,
          );
          await saveDeviceSession({ apiBaseUrl, tokens: refreshed });
          setTokens(refreshed);
          return;
        } catch {
          await discardSession();
        }
        return;
      }
      setMode("error");
      setMessage(copy.messages.loadFailed);
    }
  }, [apiBaseUrl, refreshConversations, sessionLoaded, tokens]);

  async function discardSession() {
    try {
      await clearDeviceSession();
    } finally {
      setTokens(undefined);
      setConversations([]);
      setConversationMessages([]);
      setSelectedConversationId(undefined);
      setConversationJobs({});
      pendingConversationId.current = undefined;
      setMode("setup");
      setMessage(copy.messages.sessionExpired);
    }
  }

  useEffect(() => {
    let current = true;

    void readDeviceSession()
      .then((stored) => {
        if (!current) return;
        if (stored) {
          setApiBaseUrl(stored.apiBaseUrl);
          setTokens(stored.tokens);
          setMode("loading");
        } else {
          setMode("setup");
        }
      })
      .catch(() => {
        if (current) {
          setMode("setup");
          setMessage(copy.messages.storageNotice);
        }
      })
      .finally(() => {
        if (current) setSessionLoaded(true);
      });

    return () => {
      current = false;
    };
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const activeJobIds = useMemo(
    () =>
      Object.values(conversationJobs)
        .filter((job) => !isTerminalAgentJob(job.state))
        .map((job) => job.id)
        .sort(),
    [conversationJobs],
  );
  const activeJobKey = activeJobIds.join(":");

  useEffect(() => {
    if (!tokens || activeJobIds.length === 0) return;
    let current = true;

    const poll = async () => {
      const results = await Promise.all(
        activeJobIds.map(async (jobId) => {
          try {
            return await fetchAgentJob(apiBaseUrl, tokens.accessToken, jobId);
          } catch {
            return undefined;
          }
        }),
      );
      if (!current) return;
      const jobs = results.filter((job): job is AgentJob => Boolean(job));
      if (jobs.length !== activeJobIds.length) {
        setConversationError(copy.messages.conversationLoadNotice);
      }
      if (jobs.length === 0) return;
      setConversationJobs((known) => {
        const next = { ...known };
        for (const job of jobs) next[job.conversationId] = job;
        return next;
      });
      const finishedConversationIds = jobs
        .filter((job) => isTerminalAgentJob(job.state))
        .map((job) => job.conversationId);
      if (finishedConversationIds.includes(selectedConversationId ?? "")) {
        void loadConversationMessages(selectedConversationId!);
      }
      if (finishedConversationIds.length) void refreshConversations();
    };

    void poll();
    const interval = window.setInterval(() => void poll(), 1_500);
    return () => {
      current = false;
      window.clearInterval(interval);
    };
  }, [
    activeJobKey,
    apiBaseUrl,
    loadConversationMessages,
    refreshConversations,
    selectedConversationId,
    tokens,
  ]);

  const today = useMemo(
    () =>
      new Intl.DateTimeFormat("ko-KR", {
        month: "long",
        day: "numeric",
        weekday: "short",
      }).format(new Date()),
    [],
  );

  async function pairDevice(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const pairingCode = String(form.get("pairingCode") ?? "").trim();
    const deviceName = String(form.get("deviceName") ?? "").trim();
    if (!apiBaseUrl.trim() || !pairingCode || !deviceName) {
      setMessage(copy.messages.setupRequired);
      return;
    }
    setMode("loading");
    try {
      const nextTokens = await exchangePairingCode(
        apiBaseUrl,
        pairingCode,
        deviceName,
        await readOrCreateInstallationId(),
      );
      await saveDeviceSession({
        apiBaseUrl,
        tokens: nextTokens,
      });
      setTokens(nextTokens);
      setMessage(undefined);
    } catch {
      setMode("setup");
      setMessage(copy.messages.connectionNotice);
    }
  }

  async function addTask(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!tokens) return;
    const form = new FormData(event.currentTarget);
    const title = String(form.get("title") ?? "").trim();
    if (!title) return;
    try {
      const task = await createTask(apiBaseUrl, tokens.accessToken, {
        title,
        priority: 1,
      });
      setTasks((current) => [...current, task]);
      event.currentTarget.reset();
      setMessage(copy.messages.taskAdded);
    } catch {
      setMessage(copy.messages.saveFailed);
    }
  }

  async function finishTask(task: Task) {
    if (!tokens) return;
    try {
      await completeTask(apiBaseUrl, tokens.accessToken, task);
      setTasks((current) => current.filter((item) => item.id !== task.id));
      setMessage(copy.messages.taskCompleted);
    } catch (error) {
      setMessage(
        error instanceof PlanningRequestError && error.code === "conflict"
          ? copy.messages.taskChanged
          : copy.messages.saveFailed,
      );
    }
  }

  async function addSchedule(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!tokens) return;
    const form = new FormData(event.currentTarget);
    const title = String(form.get("scheduleTitle") ?? "").trim();
    const startsAt = String(form.get("startsAt") ?? "");
    const endsAt = String(form.get("endsAt") ?? "");
    if (!title || !startsAt || !endsAt) return;
    try {
      const entry = await createScheduleEntry(apiBaseUrl, tokens.accessToken, {
        title,
        startsAt,
        endsAt,
      });
      setSchedule((current) =>
        [...current, entry].sort((left, right) =>
          left.startsAt.localeCompare(right.startsAt),
        ),
      );
      event.currentTarget.reset();
      setMessage(copy.messages.scheduleAdded);
    } catch {
      setMessage(copy.messages.saveFailed);
    }
  }

  function selectConversation(conversationId: string) {
    setSelectedConversationId(conversationId);
    setConversationMessages([]);
    void loadConversationMessages(conversationId);
    void restoreConversationJob(conversationId);
  }

  async function restoreConversationJob(conversationId: string) {
    if (!tokens) return;
    try {
      const job = await fetchLatestConversationJob(
        apiBaseUrl,
        tokens.accessToken,
        conversationId,
      );
      if (job) {
        setConversationJobs((known) => ({
          ...known,
          [conversationId]: job,
        }));
      }
    } catch {
      setConversationError(copy.messages.conversationLoadNotice);
    }
  }

  function startConversation() {
    setSelectedConversationId(undefined);
    setConversationMessages([]);
    setConversationError(undefined);
    pendingConversationId.current = undefined;
  }

  async function sendConversationRequest(
    text: string,
    clientMessageId: string,
  ): Promise<boolean> {
    if (!tokens) return false;
    let conversationId = selectedConversationId;
    setConversationError(undefined);
    try {
      if (!conversationId) {
        const clientConversationId =
          pendingConversationId.current ?? createUuidV7();
        pendingConversationId.current = clientConversationId;
        const conversation = await createConversation(
          apiBaseUrl,
          tokens.accessToken,
          clientConversationId,
          conversationTitle(text),
        );
        pendingConversationId.current = undefined;
        conversationId = conversation.id;
        setConversations((current) => [conversation, ...current]);
        setSelectedConversationId(conversation.id);
      }
      const queued = await queueAgentTurn(
        apiBaseUrl,
        tokens.accessToken,
        conversationId,
        text.trim(),
        clientMessageId,
      );
      setConversationJobs((known) => ({
        ...known,
        [queued.conversationId]: {
          id: queued.jobId,
          conversationId: queued.conversationId,
          state: queued.state,
          createdAt: new Date().toISOString(),
          finishedAt: null,
          version: 1,
        },
      }));
      await loadConversationMessages(queued.conversationId);
      void refreshConversations();
      return true;
    } catch (error) {
      setConversationError(
        error instanceof AgentRequestError && error.code === "conflict"
          ? copy.messages.conversationBusy
          : copy.messages.conversationSendNotice,
      );
      return false;
    }
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="app-header__inner">
          <div className="brand">
            <span className="brand__mark" aria-hidden="true">
              J
            </span>
            <span className="brand__name">Jimin OS</span>
          </div>
          <div className="app-header__controls">
            {tokens && (
              <nav className="app-nav" aria-label="주요 메뉴">
                <button
                  className="app-nav__item focus-visible-control"
                  data-current={view === "today"}
                  type="button"
                  onClick={() => setView("today")}
                >
                  {copy.navigation.today}
                </button>
                <button
                  className="app-nav__item focus-visible-control"
                  data-current={view === "conversations"}
                  type="button"
                  onClick={() => setView("conversations")}
                >
                  {copy.navigation.conversations}
                </button>
              </nav>
            )}
            <button
              className="quiet-button focus-visible-control"
              type="button"
              aria-label={copy.actions.refresh}
              onClick={() => void refresh()}
              disabled={!tokens || mode === "loading"}
            >
              <RefreshCw aria-hidden="true" />
              <span className="refresh-label">{copy.actions.refresh}</span>
            </button>
          </div>
        </div>
      </header>
      <main
        className={
          mode === "setup"
            ? "setup-main"
            : view === "today"
              ? "planning-page"
              : "conversation-main"
        }
      >
        {mode === "setup" ? (
          <>
            {message && (
              <p className="inline-alert" role="alert">
                {message}
              </p>
            )}
            <SetupPanel
              apiBaseUrl={apiBaseUrl}
              onApiBaseUrlChange={setApiBaseUrl}
              onSubmit={pairDevice}
            />
          </>
        ) : view === "conversations" ? (
          <ConversationWorkspace
            conversations={conversations}
            messages={conversationMessages}
            selectedConversationId={selectedConversationId}
            jobState={
              selectedConversationId
                ? conversationJobs[selectedConversationId]?.state
                : undefined
            }
            hasActiveJob={Boolean(
              selectedConversationId &&
              conversationJobs[selectedConversationId] &&
              !isTerminalAgentJob(
                conversationJobs[selectedConversationId].state,
              ),
            )}
            loading={conversationLoading}
            error={conversationError}
            onSelect={selectConversation}
            onStartConversation={startConversation}
            onSend={sendConversationRequest}
          />
        ) : (
          <>
            <section className="page-heading">
              <div>
                <p className="page-heading__date">{today}</p>
                <h1>{copy.title}</h1>
              </div>
              {tokens && (
                <button
                  className="primary-button focus-visible-control"
                  type="button"
                  onClick={() => document.getElementById("task-title")?.focus()}
                >
                  <CirclePlus aria-hidden="true" />
                  {copy.actions.addTask}
                </button>
              )}
            </section>
            {message && (
              <p
                className="inline-alert"
                role={mode === "error" ? "alert" : "status"}
                aria-live="polite"
              >
                {message}
              </p>
            )}
            <section className="planning-layout" aria-busy={mode === "loading"}>
              <section className="panel" aria-labelledby="schedule-title">
                <div className="panel__header">
                  <div>
                    <h2 id="schedule-title">
                      <CalendarDays aria-hidden="true" />
                      {copy.schedule.title}
                    </h2>
                    <p>{copy.schedule.description}</p>
                  </div>
                </div>
                {mode === "loading" ? (
                  <LoadingRows />
                ) : schedule.length ? (
                  <ol className="planning-list">
                    {schedule.map((entry) => (
                      <li key={entry.id} className="schedule-row">
                        <time dateTime={entry.startsAt}>
                          {formatTime(entry.startsAt)}
                        </time>
                        <div>
                          <strong>{entry.title}</strong>
                          <p>{formatRange(entry.startsAt, entry.endsAt)}</p>
                        </div>
                      </li>
                    ))}
                  </ol>
                ) : (
                  <EmptyState text={copy.schedule.empty} />
                )}
              </section>
              <section className="panel" aria-labelledby="task-list-title">
                <div className="panel__header">
                  <div>
                    <h2 id="task-list-title">
                      <ListTodo aria-hidden="true" />
                      {copy.tasks.title}
                    </h2>
                    <p>{copy.tasks.description}</p>
                  </div>
                </div>
                {mode === "loading" ? (
                  <LoadingRows />
                ) : tasks.length ? (
                  <ul className="planning-list">
                    {tasks.map((task) => (
                      <li key={task.id} className="task-row">
                        <button
                          className="focus-visible-control"
                          type="button"
                          aria-label={`${task.title} ${copy.actions.complete}`}
                          onClick={() => void finishTask(task)}
                        >
                          <Check aria-hidden="true" />
                        </button>
                        <span>{task.title}</span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <EmptyState text={copy.tasks.empty} />
                )}
              </section>
            </section>
            <section className="entry-forms">
              <form className="panel compact-form" onSubmit={addTask}>
                <h2>{copy.forms.taskTitle}</h2>
                <label htmlFor="task-title">{copy.forms.taskLabel}</label>
                <div className="form-action">
                  <input id="task-title" name="title" maxLength={200} />
                  <button
                    className="primary-button focus-visible-control"
                    type="submit"
                  >
                    {copy.actions.addTask}
                  </button>
                </div>
              </form>
              <form className="panel compact-form" onSubmit={addSchedule}>
                <h2>{copy.forms.scheduleTitle}</h2>
                <label htmlFor="schedule-title-input">
                  {copy.forms.scheduleLabel}
                </label>
                <input
                  id="schedule-title-input"
                  name="scheduleTitle"
                  maxLength={200}
                />
                <div className="date-fields">
                  <input
                    aria-label={copy.forms.startsAt}
                    name="startsAt"
                    type="datetime-local"
                    required
                  />
                  <input
                    aria-label={copy.forms.endsAt}
                    name="endsAt"
                    type="datetime-local"
                    required
                  />
                </div>
                <button
                  className="secondary-button focus-visible-control"
                  type="submit"
                >
                  {copy.actions.addSchedule}
                </button>
              </form>
            </section>
          </>
        )}
      </main>
    </div>
  );
}

function SetupPanel({
  apiBaseUrl,
  onApiBaseUrlChange,
  onSubmit,
}: {
  apiBaseUrl: string;
  onApiBaseUrlChange(value: string): void;
  onSubmit(event: FormEvent<HTMLFormElement>): void;
}) {
  return (
    <section className="setup-panel" aria-labelledby="setup-title">
      <div className="setup-panel__intro">
        <Server aria-hidden="true" />
        <p className="setup-panel__eyebrow">{copy.setup.eyebrow}</p>
        <h1 id="setup-title">{copy.setup.title}</h1>
        <p className="setup-panel__description">{copy.setup.description}</p>
      </div>
      <aside className="setup-panel__scope" aria-label={copy.setup.scopeTitle}>
        <strong>{copy.setup.scopeTitle}</strong>
        <p>{copy.setup.scopeDescription}</p>
      </aside>
      <ol className="setup-steps" aria-label={copy.setup.preparationLabel}>
        <li>
          <span aria-hidden="true">1</span>
          <div>
            <strong>{copy.setup.prepareServerTitle}</strong>
            <p>{copy.setup.prepareServerDescription}</p>
          </div>
        </li>
        <li>
          <span aria-hidden="true">2</span>
          <div>
            <strong>{copy.setup.prepareCodeTitle}</strong>
            <p>{copy.setup.prepareCodeDescription}</p>
          </div>
        </li>
      </ol>
      <form className="setup-form" onSubmit={onSubmit}>
        <div className="field">
          <label htmlFor="server-url">{copy.setup.serverLabel}</label>
          <p id="server-url-hint" className="field__hint">
            {copy.setup.serverHint}
          </p>
          <input
            id="server-url"
            type="url"
            value={apiBaseUrl}
            required
            aria-describedby="server-url-hint"
            onChange={(event) => onApiBaseUrlChange(event.target.value)}
          />
        </div>
        <div className="field">
          <label htmlFor="device-name">{copy.setup.deviceLabel}</label>
          <p id="device-name-hint" className="field__hint">
            {copy.setup.deviceHint}
          </p>
          <input
            id="device-name"
            name="deviceName"
            defaultValue={copy.setup.defaultDeviceName}
            maxLength={80}
            required
            aria-describedby="device-name-hint"
          />
        </div>
        <div className="field">
          <label htmlFor="pairing-code">{copy.setup.tokenLabel}</label>
          <p id="pairing-code-hint" className="field__hint">
            {copy.setup.tokenHint}
          </p>
          <textarea
            id="pairing-code"
            name="pairingCode"
            required
            rows={3}
            aria-describedby="pairing-code-hint"
          />
        </div>
        <button className="primary-button focus-visible-control" type="submit">
          {copy.actions.connect}
        </button>
      </form>
    </section>
  );
}
function EmptyState({ text }: { text: string }) {
  return <p className="empty-state">{text}</p>;
}
function LoadingRows() {
  return (
    <div className="loading-rows">
      <span className="skeleton" />
      <span className="skeleton" />
      <span className="skeleton" />
    </div>
  );
}
function formatTime(value: string) {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}
function formatRange(start: string, end: string) {
  return `${formatTime(start)} – ${formatTime(end)}`;
}

function conversationTitle(value: string) {
  const title = value.trim().replace(/\s+/g, " ").slice(0, 36);
  return title || null;
}

function isTerminalAgentJob(state: AgentJob["state"]) {
  return ["completed", "failed", "cancelled", "declined"].includes(state);
}
