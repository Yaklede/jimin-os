import {
  CalendarClock,
  ChevronRight,
  History,
  Pause,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  SkipForward,
  Trash2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createUuidV7 } from "../uuid";
import { registerMobileBackHandler } from "../mobileBack";
import { PlanningRequestError } from "../api/planning";
import {
  fetchProjects,
  fetchWorkspaces,
  type Project,
  type Workspace,
} from "../api/projects";
import { fetchProjectWebhooks, type ProjectWebhook } from "../api/webhooks";
import {
  scheduledRequest,
  type ScheduledWork,
  type ScheduledWorkDefinition,
  type ScheduledWorkPreview,
  type ScheduledWorkRunPage,
} from "../api/scheduledWork";
import { scheduledWorkCopy as c } from "../copy/scheduledWork";
import {
  latestScheduledFailure,
  mergeScheduledRuns,
  parseAssigneeFilter,
} from "../scheduledWorkState";
import "./scheduledWork.css";

interface Props {
  baseUrl: string;
  authenticate<T>(request: (token: string) => Promise<T>): Promise<T>;
  workspaceId?: string;
  projectId?: string;
}
const days = ["월", "화", "수", "목", "금", "토", "일"];
const date = (value: string) =>
  new Intl.DateTimeFormat("ko-KR", {
    timeZone: "Asia/Seoul",
    month: "short",
    day: "numeric",
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
export function scheduledWorkTemplate(
  workspaceId: string,
  template: "deadline" | "followup" | "brief" = "deadline",
): ScheduledWorkDefinition {
  return {
    title:
      template === "brief"
        ? "내일 준비 브리핑"
        : template === "followup"
          ? "마감 안내와 오후 재확인"
          : "오늘 마감 안내",
    workspaceId,
    projectId: null,
    webhookId: null,
    weekdays: [1, 2, 3, 4, 5],
    time: template === "brief" ? "18:00" : "09:00",
    followUpTime: template === "followup" ? "16:00" : null,
    timeZone: "Asia/Seoul",
    taskScope: template === "brief" ? "tomorrow" : "today",
    includeOverdue: true,
    assigneeNames: [],
    destination: "in_app",
    mentionAssignees: false,
    mentionNames: [],
    includeSchedules: template === "brief",
  };
}
function errorCopy(error: unknown) {
  if (error instanceof PlanningRequestError && error.code === "conflict")
    return c.errors.conflict;
  if (error instanceof PlanningRequestError && error.code === "invalid")
    return c.errors.invalid;
  return c.errors.unavailable;
}

export function ScheduledWorkPanel({
  baseUrl,
  authenticate,
  workspaceId,
  projectId,
}: Props) {
  const [items, setItems] = useState<ScheduledWork[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [webhooks, setWebhooks] = useState<ProjectWebhook[]>([]);
  const [runs, setRuns] = useState<ScheduledWorkRunPage>({
    items: [],
    nextCursor: null,
  });
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<"list" | "history">("list");
  const [editing, setEditing] = useState<ScheduledWork>();
  const [definition, setDefinition] = useState<ScheduledWorkDefinition>();
  const [assigneeFilter, setAssigneeFilter] = useState("");
  const [preview, setPreview] = useState<ScheduledWorkPreview>();
  const [previewKey, setPreviewKey] = useState("");
  const [initialKey, setInitialKey] = useState("");
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirm, setConfirm] = useState<
    "close" | "list" | "delete" | undefined
  >();
  const [deleteItem, setDeleteItem] = useState<ScheduledWork>();
  const dialog = useRef<HTMLDialogElement>(null);
  const confirmation = useRef<HTMLElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const generation = useRef(0);
  const runKeys = useRef(new Map<string, string>());
  const newId = useRef(createUuidV7());
  useEffect(() => {
    if (!confirm) return;
    confirmation.current?.scrollIntoView({ block: "nearest" });
    confirmation.current?.querySelector("button")?.focus();
  }, [confirm]);
  const request = useCallback(
    <T,>(path = "", body?: unknown, method = "GET") =>
      authenticate((token) =>
        scheduledRequest<T>(baseUrl, token, path, body, method),
      ),
    [authenticate, baseUrl],
  );
  const reload = useCallback(async () => {
    const version = ++generation.current;
    try {
      const [list, spaces, history] = await Promise.all([
        request<{ items: ScheduledWork[] }>(),
        authenticate((token) => fetchWorkspaces(baseUrl, token)),
        request<ScheduledWorkRunPage>("/runs"),
      ]);
      if (version !== generation.current) return;
      setItems(list.items);
      setWorkspaces(spaces);
      setRuns((current) => mergeScheduledRuns(current, history));
    } catch (failure) {
      if (version === generation.current) setError(errorCopy(failure));
    }
  }, [request, authenticate, baseUrl]);
  useEffect(() => {
    void reload();
    const interval = window.setInterval(() => {
      if (!document.hidden) void reload();
    }, 30_000);
    return () => {
      ++generation.current;
      window.clearInterval(interval);
    };
  }, [reload]);
  const close = useCallback(() => {
    if (busy) return;
    if (definition && JSON.stringify(definition) !== initialKey) {
      setConfirm("close");
      return;
    }
    setOpen(false);
  }, [busy, definition, initialKey]);
  useEffect(() => {
    if (!open) {
      dialog.current?.close();
      opener.current?.focus();
      return;
    }
    opener.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    dialog.current?.showModal();
  }, [open]);
  useEffect(
    () =>
      open
        ? registerMobileBackHandler(() => {
            close();
            return true;
          }, 100)
        : undefined,
    [open, close],
  );
  useEffect(() => {
    let cancelled = false;
    setProjects([]);
    if (definition?.workspaceId)
      authenticate((token) =>
        fetchProjects(baseUrl, token, definition.workspaceId),
      )
        .then((values) => {
          if (!cancelled)
            setProjects(values.filter((p) => p.status === "active"));
        })
        .catch((failure) => {
          if (!cancelled) setError(errorCopy(failure));
        });
    return () => {
      cancelled = true;
    };
  }, [authenticate, baseUrl, definition?.workspaceId]);
  useEffect(() => {
    let cancelled = false;
    setWebhooks([]);
    if (definition?.projectId)
      authenticate((token) =>
        fetchProjectWebhooks(baseUrl, token, definition.projectId!),
      )
        .then((values) => {
          if (!cancelled)
            setWebhooks(
              values.filter((h) => h.enabled && h.provider === "google_chat"),
            );
        })
        .catch((failure) => {
          if (!cancelled) setError(errorCopy(failure));
        });
    return () => {
      cancelled = true;
    };
  }, [authenticate, baseUrl, definition?.projectId]);
  function edit(
    item?: ScheduledWork,
    template: "deadline" | "followup" | "brief" = "deadline",
  ) {
    const workspace =
      (template === "brief"
        ? workspaces.find((w) => w.scope === "personal")?.id
        : workspaceId) ??
      workspaces[0]?.id ??
      "";
    const next = item?.definition ?? {
      ...scheduledWorkTemplate(workspace, template),
      projectId: template === "brief" ? null : (projectId ?? null),
    };
    setEditing(item);
    setDefinition(next);
    setAssigneeFilter(next.assigneeNames.join(", "));
    setInitialKey(JSON.stringify(next));
    setPreview(undefined);
    setPreviewKey("");
    setError("");
    setNotice("");
    setTab("list");
    newId.current = createUuidV7();
  }
  function update(patch: Partial<ScheduledWorkDefinition>) {
    setDefinition((old) => (old ? { ...old, ...patch } : old));
    setPreview(undefined);
    setPreviewKey("");
    setNotice("");
  }
  async function perform(label: string, operation: () => Promise<void>) {
    if (busy) return;
    setBusy(label);
    setError("");
    setNotice("");
    try {
      await operation();
    } catch (failure) {
      setError(errorCopy(failure));
    } finally {
      setBusy("");
    }
  }
  async function save(enabled: boolean) {
    if (!definition) return;
    await perform("save", async () => {
      await request(
        `/${editing?.id ?? newId.current}`,
        { definition, enabled, expectedVersion: editing?.version ?? null },
        "PUT",
      );
      setDefinition(undefined);
      setEditing(undefined);
      setNotice(c.saved);
      await reload();
    });
  }
  async function action(
    item: ScheduledWork,
    kind: "run_now" | "skip_once" | "pause" | "delete",
  ) {
    await perform(item.id, async () => {
      const key = runKeys.current.get(item.id) ?? createUuidV7();
      if (kind === "run_now") runKeys.current.set(item.id, key);
      await request(
        `/${item.id}/actions`,
        {
          kind,
          expectedVersion: item.version,
          requestId: kind === "run_now" ? key : null,
        },
        "POST",
      );
      if (kind === "run_now") runKeys.current.delete(item.id);
      setConfirm(undefined);
      setNotice(kind === "run_now" ? c.queued : c.changed);
      if (kind === "run_now") setTab("history");
      await reload();
    });
  }
  const scoped = items.filter(
    (item) =>
      (!projectId || item.definition.projectId === projectId) &&
      (!workspaceId || item.definition.workspaceId === workspaceId),
  );
  const active = scoped.filter((item) => item.enabled);
  const latestFailure = latestScheduledFailure(
    runs,
    scoped.map((item) => item.id),
  );
  const key = definition ? JSON.stringify(definition) : "";
  const names =
    webhooks.find((h) => h.id === definition?.webhookId)?.mentionDirectory
      .users ?? {};
  return (
    <>
      <button
        type="button"
        className="scheduled-summary focus-visible-control"
        onClick={() => {
          setOpen(true);
          setDefinition(undefined);
          setConfirm(undefined);
          void reload();
        }}
      >
        <CalendarClock aria-hidden="true" />
        <span>
          <strong>
            {c.title}
            {active.length > 0 ? ` · ${active.length}개 실행 중` : ""}
          </strong>
          <small>
            {latestFailure
              ? "확인할 실행 결과가 있어요."
              : active[0]
                ? `다음 실행 ${date(active[0].nextRunAt)}`
                : c.description}
          </small>
        </span>
        <ChevronRight aria-hidden="true" />
      </button>
      <dialog
        ref={dialog}
        className="scheduled-dialog"
        aria-labelledby="scheduled-title"
        onCancel={(event) => {
          event.preventDefault();
          close();
        }}
      >
        <header className="scheduled-heading">
          <div>
            <h2 id="scheduled-title">{c.title}</h2>
            <p>{c.description}</p>
          </div>
          <button
            type="button"
            className="scheduled-icon"
            aria-label="예약 업무 닫기"
            disabled={!!busy}
            onClick={close}
          >
            <X />
          </button>
        </header>
        <div className="scheduled-toolbar">
          <div className="scheduled-tabs">
            <button
              type="button"
              aria-pressed={tab === "list"}
              onClick={() => setTab("list")}
            >
              {c.list}
            </button>
            <button
              type="button"
              aria-pressed={tab === "history"}
              onClick={() => {
                setTab("history");
                void reload();
              }}
            >
              <History />
              {c.history}
            </button>
          </div>
          <button
            type="button"
            className="scheduled-icon"
            aria-label="예약 새로고침"
            disabled={!!busy}
            onClick={() => void perform("reload", reload)}
          >
            <RefreshCw className={busy === "reload" ? "scheduled-spin" : ""} />
          </button>
        </div>
        {error && (
          <p className="inline-alert" role="alert">
            {error}
          </p>
        )}
        {notice && (
          <p className="scheduled-notice" role="status">
            {notice}
          </p>
        )}
        {confirm && (
          <section
            ref={confirmation}
            className="scheduled-confirm"
            aria-label="변경 확인"
          >
            <p>{confirm === "delete" ? c.deleteHelp : c.closeHelp}</p>
            <div className="scheduled-actions">
              <button type="button" onClick={() => setConfirm(undefined)}>
                돌아가기
              </button>
              <button
                type="button"
                disabled={!!busy}
                onClick={() =>
                  confirm === "list"
                    ? (setDefinition(undefined),
                      setEditing(undefined),
                      setConfirm(undefined))
                    : confirm === "close"
                      ? (setConfirm(undefined),
                        setDefinition(undefined),
                        setOpen(false))
                      : deleteItem && void action(deleteItem, "delete")
                }
              >
                {confirm === "delete" ? "예약 지우기" : "저장하지 않고 닫기"}
              </button>
            </div>
          </section>
        )}
        {tab === "list" && !definition && (
          <div className="scheduled-content">
            <div className="scheduled-actions">
              <button
                className="primary-button"
                type="button"
                disabled={workspaces.length === 0}
                onClick={() => edit()}
              >
                <Plus />
                {c.add}
              </button>
              <button
                type="button"
                disabled={workspaces.length === 0}
                onClick={() => edit(undefined, "brief")}
              >
                저녁 브리핑 만들기
              </button>
            </div>
            {!scoped.length && <p className="scheduled-empty">{c.empty}</p>}
            {scoped.map((item) => (
              <article className="scheduled-rule" key={item.id}>
                <div>
                  <strong>{item.definition.title}</strong>
                  <span className="scheduled-badge">
                    {item.enabled ? c.active : c.paused}
                  </span>
                  <p>
                    {item.definition.weekdays
                      .map((day) => days[day - 1])
                      .join("·")}{" "}
                    {item.definition.time}
                    {item.definition.followUpTime
                      ? ` / ${item.definition.followUpTime} 재확인`
                      : ""}{" "}
                    · 한국 시간
                  </p>
                  <p>
                    {item.definition.destination === "google_chat"
                      ? "Google Chat"
                      : "홈과 앱 알림"}{" "}
                    ·{" "}
                    {item.enabled
                      ? `다음 ${date(item.nextRunAt)}`
                      : "다시 시작할 때까지 실행하지 않아요."}
                  </p>
                </div>
                <div className="scheduled-actions">
                  <button
                    type="button"
                    disabled={!!busy}
                    onClick={() => edit(item)}
                  >
                    <Pencil />
                    수정
                  </button>
                  <button
                    type="button"
                    disabled={!!busy}
                    onClick={() => void action(item, "run_now")}
                  >
                    <Play />
                    지금 실행
                  </button>
                  {item.enabled ? (
                    <>
                      <button
                        type="button"
                        disabled={!!busy}
                        onClick={() => void action(item, "skip_once")}
                      >
                        <SkipForward />
                        이번만 건너뛰기
                      </button>
                      <button
                        type="button"
                        disabled={!!busy}
                        onClick={() => void action(item, "pause")}
                      >
                        <Pause />
                        정지
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      disabled={!!busy}
                      onClick={() => edit(item)}
                    >
                      <Play />
                      확인하고 시작
                    </button>
                  )}
                  <button
                    type="button"
                    aria-label={`${item.definition.title} 지우기`}
                    disabled={!!busy}
                    onClick={() => {
                      setDeleteItem(item);
                      setConfirm("delete");
                    }}
                  >
                    <Trash2 />
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
        {tab === "list" && definition && (
          <form
            className="scheduled-form"
            onSubmit={(event) => {
              event.preventDefault();
              void perform("preview", async () => {
                const value = await request<ScheduledWorkPreview>(
                  "/preview",
                  definition,
                  "POST",
                );
                setPreview(value);
                setPreviewKey(key);
              });
            }}
          >
            <fieldset disabled={!!busy}>
              <legend>{editing ? "예약 수정" : "새 예약"}</legend>
              {!editing && (
                <label>
                  시작 양식
                  <select
                    value={
                      definition.includeSchedules
                        ? "brief"
                        : definition.followUpTime
                          ? "followup"
                          : "deadline"
                    }
                    onChange={(event) =>
                      edit(
                        undefined,
                        event.target.value as "deadline" | "followup" | "brief",
                      )
                    }
                  >
                    <option value="deadline">마감 안내</option>
                    <option value="followup">
                      마감 안내 + 오후 남은 일 재확인
                    </option>
                    <option value="brief">저녁에 내일 준비 브리핑</option>
                  </select>
                </label>
              )}
              <label>
                예약 이름
                <input
                  required
                  maxLength={120}
                  value={definition.title}
                  onChange={(event) => update({ title: event.target.value })}
                />
              </label>
              <div className="scheduled-grid">
                <label>
                  업무 공간
                  <select
                    required
                    value={definition.workspaceId}
                    onChange={(event) =>
                      update({
                        workspaceId: event.target.value,
                        projectId: null,
                        webhookId: null,
                        includeSchedules: false,
                      })
                    }
                  >
                    {workspaces.map((space) => (
                      <option key={space.id} value={space.id}>
                        {space.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  프로젝트
                  <select
                    value={definition.projectId ?? ""}
                    required={definition.destination === "google_chat"}
                    onChange={(event) =>
                      update({
                        projectId: event.target.value || null,
                        webhookId: null,
                      })
                    }
                  >
                    <option value="">공간 전체</option>
                    {projects.map((project) => (
                      <option key={project.id} value={project.id}>
                        {project.title}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <fieldset className="scheduled-days">
                <legend>반복 요일</legend>
                {days.map((day, index) => (
                  <button
                    type="button"
                    aria-pressed={definition.weekdays.includes(index + 1)}
                    aria-label={`${day}요일`}
                    key={day}
                    onClick={() =>
                      update({
                        weekdays: definition.weekdays.includes(index + 1)
                          ? definition.weekdays.filter((d) => d !== index + 1)
                          : [...definition.weekdays, index + 1].sort(),
                      })
                    }
                  >
                    {day}
                  </button>
                ))}
              </fieldset>
              <div className="scheduled-grid">
                <label>
                  실행 시각
                  <input
                    type="time"
                    required
                    value={definition.time}
                    onChange={(event) => update({ time: event.target.value })}
                  />
                  <small>한국 시간 기준이에요.</small>
                </label>
                <label>
                  후속 확인 시각 (선택)
                  <input
                    type="time"
                    value={definition.followUpTime ?? ""}
                    disabled={definition.includeSchedules}
                    onChange={(event) =>
                      update({ followUpTime: event.target.value || null })
                    }
                  />
                  <small>앞서 안내한 일 중 남은 일만 확인해요.</small>
                </label>
              </div>
              <label>
                확인할 할 일
                <select
                  value={definition.taskScope}
                  onChange={(event) =>
                    update({
                      taskScope: event.target
                        .value as ScheduledWorkDefinition["taskScope"],
                    })
                  }
                >
                  <option value="today">오늘 마감</option>
                  <option value="tomorrow">내일 마감</option>
                  <option value="today_tomorrow">오늘과 내일 마감</option>
                  <option value="overdue">기한이 지난 일</option>
                  <option value="all_open">모든 열린 할 일</option>
                </select>
              </label>
              <label className="scheduled-check">
                <input
                  type="checkbox"
                  checked={definition.includeOverdue}
                  onChange={(event) =>
                    update({ includeOverdue: event.target.checked })
                  }
                />
                기한이 지난 일도 포함
              </label>
              <label className="scheduled-check">
                <input
                  type="checkbox"
                  disabled={
                    definition.destination === "google_chat" ||
                    !!definition.followUpTime ||
                    workspaces.find((w) => w.id === definition.workspaceId)
                      ?.scope !== "personal"
                  }
                  checked={definition.includeSchedules}
                  onChange={(event) =>
                    update({ includeSchedules: event.target.checked })
                  }
                />
                개인 일정도 함께 확인
              </label>
              <label>
                전달할 곳
                <select
                  value={definition.destination}
                  onChange={(event) =>
                    update({
                      destination: event.target.value as
                        "google_chat" | "in_app",
                      webhookId: null,
                      mentionNames: [],
                      mentionAssignees: event.target.value === "google_chat",
                      includeSchedules: false,
                    })
                  }
                >
                  <option value="in_app">홈과 앱 알림</option>
                  <option value="google_chat">Google Chat</option>
                </select>
              </label>
              {definition.destination === "google_chat" && (
                <>
                  <label>
                    프로젝트 연결
                    <select
                      required
                      value={definition.webhookId ?? ""}
                      onChange={(event) =>
                        update({
                          webhookId: event.target.value || null,
                          mentionNames: [],
                        })
                      }
                    >
                      <option value="">연결 선택</option>
                      {webhooks.map((webhook) => (
                        <option key={webhook.id} value={webhook.id}>
                          {webhook.destinationLabel}
                        </option>
                      ))}
                    </select>
                  </label>
                  {!webhooks.length && (
                    <p>프로젝트에 Google Chat 웹훅을 먼저 연결해 주세요.</p>
                  )}
                  <label className="scheduled-check">
                    <input
                      type="checkbox"
                      checked={definition.mentionAssignees}
                      onChange={(event) =>
                        update({ mentionAssignees: event.target.checked })
                      }
                    />
                    할 일의 현재 담당자 멘션
                  </label>
                  <fieldset>
                    <legend>추가로 멘션할 사람</legend>
                    <div className="scheduled-people">
                      {Object.keys(names).map((name) => (
                        <label className="scheduled-check" key={name}>
                          <input
                            type="checkbox"
                            checked={definition.mentionNames.includes(name)}
                            onChange={(event) =>
                              update({
                                mentionNames: event.target.checked
                                  ? [...definition.mentionNames, name]
                                  : definition.mentionNames.filter(
                                      (n) => n !== name,
                                    ),
                              })
                            }
                          />
                          {name}
                        </label>
                      ))}
                    </div>
                  </fieldset>
                </>
              )}
              <label>
                특정 담당자의 일만 확인 (선택)
                <input
                  value={assigneeFilter}
                  placeholder="예: 김경주, 주홍석"
                  onChange={(event) => {
                    setAssigneeFilter(event.target.value);
                    update({
                      assigneeNames: parseAssigneeFilter(event.target.value),
                    });
                  }}
                />
                <small>비워 두면 모든 담당자의 일을 확인해요.</small>
              </label>
            </fieldset>
            <p className="scheduled-muted">{c.previewHelp}</p>
            <div className="scheduled-actions">
              <button
                type="submit"
                disabled={!!busy || !definition.weekdays.length}
              >
                {busy === "preview" ? "내용 확인 중…" : c.preview}
              </button>
              <button
                type="button"
                disabled={!!busy}
                onClick={() => void save(false)}
              >
                {c.draft}
              </button>
            </div>
            {preview && (
              <section className="scheduled-preview" aria-label="전송 미리보기">
                <h3>
                  할 일 {preview.taskIds.length}개 · 일정{" "}
                  {preview.scheduleCount}개
                </h3>
                <p>다음 실행: {date(preview.nextRunAt)}</p>
                {preview.warnings.map((warning) => (
                  <p role="alert" className="inline-alert" key={warning}>
                    {warning}
                  </p>
                ))}
                {!preview.messages.length && <p>{c.noMatches}</p>}
                {preview.messages.map((message, index) => (
                  <details key={index} open={index === 0}>
                    <summary>
                      전달 내용 {index + 1}/{preview.messages.length}
                    </summary>
                    <pre>{message}</pre>
                  </details>
                ))}
              </section>
            )}
            <footer className="scheduled-actions">
              <button
                type="button"
                disabled={!!busy}
                onClick={() => {
                  if (key !== initialKey) {
                    setConfirm("list");
                    return;
                  }
                  setDefinition(undefined);
                  setEditing(undefined);
                }}
              >
                목록으로
              </button>
              <button
                className="primary-button"
                type="button"
                disabled={
                  !!busy ||
                  !preview ||
                  previewKey !== key ||
                  !!preview.warnings.length
                }
                onClick={() => void save(true)}
              >
                {busy === "save" ? "저장 중…" : c.activate}
              </button>
            </footer>
          </form>
        )}
        {tab === "history" && (
          <div className="scheduled-content">
            {!runs.items.length && <p>아직 실행한 예약이 없어요.</p>}
            {runs.items
              .filter(
                (run) =>
                  (!projectId && !workspaceId) ||
                  scoped.some((item) => item.id === run.scheduledWorkId),
              )
              .map((run) => (
                <article className="scheduled-rule" key={run.id}>
                  <strong>
                    {items.find((item) => item.id === run.scheduledWorkId)
                      ?.definition.title ?? c.title}
                  </strong>
                  <span className="scheduled-badge" data-status={run.status}>
                    {
                      {
                        delivering: "전송 중",
                        completed: "처리 완료",
                        skipped: "건너뜀",
                        failed: "확인 필요",
                      }[run.status]
                    }
                  </span>
                  <p>
                    {date(run.scheduledFor)} ·{" "}
                    {run.kind === "followup"
                      ? "후속 확인"
                      : run.kind === "manual"
                        ? "직접 실행"
                        : "예약 실행"}{" "}
                    · 할 일 {run.taskIds.length}개
                  </p>
                  {run.reason && (
                    <p>
                      {c.reasons[run.reason] ?? "실행 결과를 확인해 주세요."}
                    </p>
                  )}
                  {run.messages.length > 0 && (
                    <details>
                      <summary>결과 펼치기</summary>
                      {run.messages.map((message, index) => (
                        <pre key={index}>{message}</pre>
                      ))}
                    </details>
                  )}
                </article>
              ))}
            {runs.nextCursor && (
              <button
                type="button"
                disabled={!!busy}
                onClick={() =>
                  void perform("history", async () => {
                    const page = await request<ScheduledWorkRunPage>(
                      `/runs?before=${runs.nextCursor}`,
                    );
                    setRuns((current) => ({
                      ...page,
                      items: [...current.items, ...page.items],
                    }));
                  })
                }
              >
                이전 실행 더 보기
              </button>
            )}
          </div>
        )}
      </dialog>
    </>
  );
}
