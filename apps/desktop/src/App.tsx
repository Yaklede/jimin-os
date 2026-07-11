import {
  CalendarDays,
  Check,
  CirclePlus,
  ListTodo,
  RefreshCw,
  Server,
} from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";

import {
  PlanningRequestError,
  completeTask,
  createScheduleEntry,
  createTask,
  exchangePairingCode,
  fetchPlanning,
  type ScheduleEntry,
  type Task,
} from "./api/planning";
import { copy } from "./copy";

const defaultApiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? "/server";
const sessionKey = "jimin-os-dev-session";

type AppMode = "setup" | "loading" | "ready" | "error";

export default function App() {
  const [apiBaseUrl, setApiBaseUrl] = useState(defaultApiBaseUrl);
  const [tokens, setTokens] = useState<
    | {
        accessToken: string;
        refreshToken: string;
      }
    | undefined
  >(readStoredSession);
  const [mode, setMode] = useState<AppMode>(tokens ? "loading" : "setup");
  const [schedule, setSchedule] = useState<ScheduleEntry[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [message, setMessage] = useState<string | undefined>(undefined);

  const refresh = useCallback(async () => {
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
    } catch (error) {
      if (
        error instanceof PlanningRequestError &&
        error.code === "unauthorized"
      ) {
        sessionStorage.removeItem(sessionKey);
        setTokens(undefined);
        setMode("setup");
        setMessage(copy.messages.sessionExpired);
        return;
      }
      setMode("error");
      setMessage(copy.messages.loadFailed);
    }
  }, [apiBaseUrl, tokens]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

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
    if (!pairingCode || !deviceName) {
      setMessage(copy.messages.setupRequired);
      return;
    }
    setMode("loading");
    try {
      const nextTokens = await exchangePairingCode(
        apiBaseUrl,
        pairingCode,
        deviceName,
      );
      sessionStorage.setItem(sessionKey, JSON.stringify(nextTokens));
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
          <button
            className="quiet-button focus-visible-control"
            type="button"
            onClick={() => void refresh()}
            disabled={!tokens || mode === "loading"}
          >
            <RefreshCw aria-hidden="true" />
            {copy.actions.refresh}
          </button>
        </div>
      </header>
      <main className="planning-page">
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
        {mode === "setup" ? (
          <SetupPanel
            apiBaseUrl={apiBaseUrl}
            onApiBaseUrlChange={setApiBaseUrl}
            onSubmit={pairDevice}
          />
        ) : (
          <>
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

function readStoredSession():
  { accessToken: string; refreshToken: string } | undefined {
  const stored = sessionStorage.getItem(sessionKey);
  if (!stored) return undefined;

  try {
    const parsed: unknown = JSON.parse(stored);
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      typeof (parsed as { accessToken?: unknown }).accessToken === "string" &&
      typeof (parsed as { refreshToken?: unknown }).refreshToken === "string"
    ) {
      return parsed as { accessToken: string; refreshToken: string };
    }
  } catch {
    // A malformed preview session is discarded below and the user can reconnect.
  }

  sessionStorage.removeItem(sessionKey);
  return undefined;
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
    <section className="setup-panel">
      <Server aria-hidden="true" />
      <div>
        <h2>{copy.setup.title}</h2>
        <p>{copy.setup.description}</p>
        <form onSubmit={onSubmit}>
          <label htmlFor="server-url">{copy.setup.serverLabel}</label>
          <input
            id="server-url"
            value={apiBaseUrl}
            onChange={(event) => onApiBaseUrlChange(event.target.value)}
          />
          <label htmlFor="device-name">{copy.setup.deviceLabel}</label>
          <input
            id="device-name"
            name="deviceName"
            defaultValue="내 Mac"
            maxLength={80}
          />
          <label htmlFor="pairing-code">{copy.setup.tokenLabel}</label>
          <textarea id="pairing-code" name="pairingCode" required rows={3} />
          <button
            className="primary-button focus-visible-control"
            type="submit"
          >
            {copy.actions.connect}
          </button>
        </form>
      </div>
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
