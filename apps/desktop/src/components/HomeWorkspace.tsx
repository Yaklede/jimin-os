import {
  AlertTriangle,
  CalendarDays,
  ChevronRight,
  Circle,
  Clock3,
  ListTodo,
  MessageCircleMore,
  Mic,
  Pencil,
  Send,
  Sparkles,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";

import { type AgentJob, type ConversationMessage } from "../api/agent";
import { type HomeSnapshot } from "../api/home";
import { type ScheduleEntry, type Task } from "../api/planning";
import { type Project } from "../api/projects";
import { presentationForMessage } from "../assistantPresentation";
import { copy } from "../copy";
import { createUuidV7 } from "../uuid";
import {
  deadlineAttentionTasks,
  taskDueState,
  type TaskDueState,
} from "../planningDue";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";
import { AssistantInteractiveCanvas } from "./AssistantInteractiveCanvas";

type HomeWorkspaceProps = {
  snapshot: HomeSnapshot | undefined;
  loading: boolean;
  error: string | undefined;
  assistantReady: boolean;
  assistantConversationId: string | undefined;
  assistantRequest: string | undefined;
  assistantJob: AgentJob | undefined;
  assistantMessage: ConversationMessage | undefined;
  onOpenAssistant(): void;
  onStartNewAssistant(): void;
  onSendAssistant(text: string, clientMessageId: string): Promise<boolean>;
  onCompleteTask(task: Task): Promise<void>;
  onEditTask(task: Task): void;
  onEditSchedule(entry: ScheduleEntry): void;
  onOpenPlanningTask(task: Task): void | Promise<void>;
  onOpenTask(task: Pick<Task, "id" | "projectId">): void | Promise<void>;
  onOpenProject(
    project: Pick<Project, "id" | "workspaceId">,
  ): void | Promise<void>;
  onOpenSchedule(
    entry: Pick<ScheduleEntry, "id" | "startsAt">,
  ): void | Promise<void>;
};

export function HomeWorkspace({
  snapshot,
  loading,
  error,
  assistantReady,
  assistantConversationId,
  assistantRequest,
  assistantJob,
  assistantMessage,
  onOpenAssistant,
  onStartNewAssistant,
  onSendAssistant,
  onCompleteTask,
  onEditTask,
  onEditSchedule,
  onOpenPlanningTask,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
}: HomeWorkspaceProps) {
  const [completingTaskId, setCompletingTaskId] = useState<string>();
  const [assistantFocused, setAssistantFocused] = useState(false);
  const [highlightedHomeTaskId, setHighlightedHomeTaskId] = useState<string>();
  const [overviewFocusTarget, setOverviewFocusTarget] = useState<
    "schedule" | "tasks"
  >();
  const highlightedHomeTaskRef = useRef<HTMLLIElement | null>(null);
  const scheduleSectionRef = useRef<HTMLElement | null>(null);
  const taskSectionRef = useRef<HTMLElement | null>(null);
  const greeting = useMemo(() => greetingForHour(new Date().getHours()), []);
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;
  const nextSchedule = snapshot?.schedule[0];
  const scheduleCount = snapshot?.schedule.length ?? 0;
  const taskCount = snapshot?.tasks.length ?? 0;
  const dueTasks = useMemo(
    () => deadlineAttentionTasks(snapshot?.dueTasks ?? []),
    [snapshot?.dueTasks],
  );
  const assistantState = homeAssistantState(
    assistantFocused,
    assistantJob,
    assistantMessage,
  );

  useEffect(() => {
    if (!overviewFocusTarget || assistantFocused) return;
    const element =
      overviewFocusTarget === "tasks"
        ? (highlightedHomeTaskRef.current ?? taskSectionRef.current)
        : scheduleSectionRef.current;
    if (!element) return;
    element.scrollIntoView({ block: "center", behavior: "smooth" });
    element.focus({ preventScroll: true });
    setOverviewFocusTarget(undefined);
  }, [assistantFocused, overviewFocusTarget]);

  async function complete(task: Task) {
    if (completingTaskId) return;
    setCompletingTaskId(task.id);
    try {
      await onCompleteTask(task);
    } finally {
      setCompletingTaskId(undefined);
    }
  }

  return (
    <section
      className="home-page"
      data-assistant-state={assistantState}
      aria-busy={showingSkeleton}
    >
      <header className="home-greeting">
        <div>
          <h1>{greeting}</h1>
          <p>{copy.home.title}</p>
        </div>
        <button
          className="home-greeting__assistant focus-visible-control"
          type="button"
          onClick={onOpenAssistant}
          aria-label={copy.actions.startAssistantConversation}
        >
          <Sparkles aria-hidden="true" />
        </button>
      </header>

      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}

      {assistantFocused && (
        <nav
          className="home-context-strip"
          aria-labelledby="home-context-strip-title"
        >
          <div className="home-context-strip__heading">
            <Sparkles aria-hidden="true" />
            <div>
              <strong id="home-context-strip-title">
                {copy.home.verifiedContextLabel}
              </strong>
              <span aria-live="polite">
                {copy.home.verifiedContextSummary(taskCount, scheduleCount)}
              </span>
            </div>
          </div>
          <div className="home-context-strip__actions">
            <button
              className="focus-visible-control"
              type="button"
              aria-label={copy.home.openTaskContext(taskCount)}
              onClick={() => {
                setHighlightedHomeTaskId(snapshot?.tasks[0]?.id);
                setOverviewFocusTarget("tasks");
                setAssistantFocused(false);
              }}
            >
              <ListTodo aria-hidden="true" />
              <span>{copy.home.taskTitle}</span>
              <strong>{taskCount}</strong>
            </button>
            <button
              className="focus-visible-control"
              type="button"
              aria-label={copy.home.openScheduleContext(scheduleCount)}
              onClick={() => {
                if (nextSchedule) {
                  onOpenSchedule(nextSchedule);
                  return;
                }
                setOverviewFocusTarget("schedule");
                setAssistantFocused(false);
              }}
            >
              <CalendarDays aria-hidden="true" />
              <span>{copy.home.scheduleTitle}</span>
              <strong>{scheduleCount}</strong>
            </button>
          </div>
        </nav>
      )}

      <HomeAssistantCommand
        ready={assistantReady}
        conversationId={assistantConversationId}
        request={assistantRequest}
        job={assistantJob}
        message={assistantMessage}
        focused={assistantFocused}
        onFocusChange={setAssistantFocused}
        onOpenAssistant={onOpenAssistant}
        onStartNew={onStartNewAssistant}
        onSend={onSendAssistant}
        onOpenTask={async (task) => {
          if (task.projectId) {
            await onOpenTask(task);
            return;
          }
          await onOpenTask(task);
          setHighlightedHomeTaskId(task.id);
          setOverviewFocusTarget("tasks");
          setAssistantFocused(false);
        }}
        onOpenProject={onOpenProject}
        onOpenSchedule={onOpenSchedule}
      />

      {!assistantFocused && (
        <>
          {!showingSkeleton && dueTasks.length > 0 && (
            <DeadlineBrief
              tasks={dueTasks}
              onEditTask={onEditTask}
              onOpenTask={onOpenPlanningTask}
            />
          )}

          <button
            className="home-briefing focus-visible-control"
            type="button"
            onClick={onOpenAssistant}
            aria-label={copy.home.askAssistant}
          >
            {showingSkeleton ? (
              <HomeBriefingSkeleton visible={skeletonVisible} />
            ) : (
              <>
                <span className="home-briefing__symbol" aria-hidden="true">
                  <Sparkles />
                </span>
                <span className="home-briefing__copy">
                  <strong>
                    {briefingHeading(nextSchedule, scheduleCount)}
                  </strong>
                  <span>{briefingSummary(scheduleCount, taskCount)}</span>
                </span>
                <ChevronRight aria-hidden="true" />
              </>
            )}
          </button>

          <button
            className="home-voice-callout focus-visible-control"
            type="button"
            onClick={onOpenAssistant}
          >
            <span className="home-voice-callout__icon" aria-hidden="true">
              <Mic />
            </span>
            <span>
              <strong>{copy.home.askAssistant}</strong>
              <span>{copy.home.description}</span>
            </span>
            <ChevronRight aria-hidden="true" />
          </button>

          <section
            className="home-next-schedule"
            aria-labelledby="next-schedule-title"
            ref={scheduleSectionRef}
            tabIndex={-1}
          >
            <div className="home-section-heading">
              <h2 id="next-schedule-title">
                {nextSchedule ? "다음 일정" : copy.home.scheduleTitle}
              </h2>
              {nextSchedule && (
                <span>{relativeScheduleTime(nextSchedule.startsAt)}</span>
              )}
            </div>
            {showingSkeleton ? (
              <ScheduleSkeleton visible={skeletonVisible} />
            ) : nextSchedule ? (
              <ScheduleHighlight
                entry={nextSchedule}
                onEdit={
                  nextSchedule.source === "manual"
                    ? () => onEditSchedule(nextSchedule)
                    : undefined
                }
              />
            ) : (
              <EmptySurface
                title={copy.home.scheduleEmptyTitle}
                description={copy.home.scheduleEmptyDescription}
              />
            )}
          </section>

          <section
            className="home-tasks"
            aria-labelledby="today-task-title"
            ref={taskSectionRef}
            tabIndex={-1}
          >
            <div className="home-section-heading">
              <h2 id="today-task-title">{copy.home.taskTitle}</h2>
              <span>
                {showingSkeleton ? (
                  <SkeletonGroup
                    className="skeleton-count"
                    label={copy.home.loadingShort}
                    visible={skeletonVisible}
                  >
                    <SkeletonBlock />
                  </SkeletonGroup>
                ) : (
                  copy.home.taskCount(taskCount)
                )}
              </span>
            </div>
            <div className="home-task-surface">
              {showingSkeleton ? (
                <TaskListSkeleton rows={4} visible={skeletonVisible} />
              ) : snapshot?.tasks.length ? (
                <ul className="home-task-list">
                  {snapshot.tasks.map((task) => (
                    <li
                      key={task.id}
                      ref={
                        highlightedHomeTaskId === task.id
                          ? highlightedHomeTaskRef
                          : undefined
                      }
                      data-highlighted={highlightedHomeTaskId === task.id}
                      tabIndex={
                        highlightedHomeTaskId === task.id ? -1 : undefined
                      }
                    >
                      <button
                        className="home-task-list__complete focus-visible-control"
                        type="button"
                        onClick={() => void complete(task)}
                        disabled={Boolean(completingTaskId)}
                        aria-label={copy.home.completeTask(task.title)}
                      >
                        {completingTaskId === task.id ? (
                          <span className="button-spinner" aria-hidden="true" />
                        ) : (
                          <Circle aria-hidden="true" />
                        )}
                      </button>
                      <span>{task.title}</span>
                      {task.dueAt && (
                        <time
                          dateTime={task.dueAt}
                          data-due-state={taskDueState(task)}
                        >
                          {taskDueLabel(task)}
                        </time>
                      )}
                      <button
                        className="home-task-list__edit focus-visible-control"
                        type="button"
                        onClick={() => onEditTask(task)}
                        disabled={Boolean(completingTaskId)}
                        aria-label={copy.home.editTask(task.title)}
                      >
                        <Pencil aria-hidden="true" />
                      </button>
                    </li>
                  ))}
                </ul>
              ) : (
                <EmptySurface
                  title={copy.home.taskEmptyTitle}
                  description={copy.home.taskEmptyDescription}
                />
              )}
            </div>
          </section>
        </>
      )}
    </section>
  );
}

function HomeAssistantCommand({
  ready,
  conversationId,
  request,
  job,
  message,
  focused,
  onFocusChange,
  onOpenAssistant,
  onStartNew,
  onSend,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
}: {
  ready: boolean;
  conversationId: string | undefined;
  request: string | undefined;
  job: AgentJob | undefined;
  message: ConversationMessage | undefined;
  focused: boolean;
  onFocusChange(focused: boolean): void;
  onOpenAssistant(): void;
  onStartNew(): void;
  onSend(text: string, clientMessageId: string): Promise<boolean>;
  onOpenTask(task: Pick<Task, "id" | "projectId">): void | Promise<void>;
  onOpenProject(
    project: Pick<Project, "id" | "workspaceId">,
  ): void | Promise<void>;
  onOpenSchedule(
    entry: Pick<ScheduleEntry, "id" | "startsAt">,
  ): void | Promise<void>;
}) {
  const [draft, setDraft] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const [submittedRequest, setSubmittedRequest] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string>();
  const inputRef = useRef<HTMLInputElement>(null);
  const focusFrameRef = useRef<number | undefined>(undefined);
  const active = Boolean(job && !isTerminalJob(job.state));

  useEffect(
    () => () => {
      if (focusFrameRef.current !== undefined) {
        cancelAnimationFrame(focusFrameRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    if (!conversationId || !request) return;
    setSubmitted(true);
    setSubmittedRequest(request);
    onFocusChange(true);
  }, [conversationId, onFocusChange, request]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const request = draft.trim();
    if (!request || sending || active || !ready) return;
    setSubmittedRequest(request);
    setSubmitted(true);
    setSending(true);
    setError(undefined);
    onFocusChange(true);
    const sent = await onSend(request, createUuidV7());
    if (sent) {
      setDraft("");
    } else {
      setError(copy.home.commandFailed);
    }
    setSending(false);
  }

  const status =
    submitted && focused && !sending && !error
      ? commandStatus(job, message)
      : undefined;
  const presentation =
    focused &&
    submitted &&
    !sending &&
    !error &&
    job?.state === "completed" &&
    message
      ? presentationForMessage(message)
      : undefined;
  const stage = presentation
    ? "result"
    : active || sending
      ? "working"
      : status
        ? "attention"
        : "idle";
  const continuing = Boolean(conversationId && submitted);
  const composer = (
    <form
      className="home-command__form"
      data-mode={continuing ? "follow-up" : "initial"}
      aria-busy={sending || active}
      onSubmit={(event) => void submit(event)}
    >
      <label className="sr-only" htmlFor="home-assistant-command">
        {continuing ? copy.home.followUpLabel : copy.home.commandLabel}
      </label>
      <input
        ref={inputRef}
        id="home-assistant-command"
        value={draft}
        maxLength={24_000}
        placeholder={
          ready
            ? continuing
              ? copy.home.followUpPlaceholder
              : copy.home.commandInputPlaceholder
            : copy.home.commandNeedsConnection
        }
        disabled={!ready || sending || active}
        onChange={(event) => {
          setDraft(event.target.value);
          setError(undefined);
        }}
      />
      <button
        className="primary-button focus-visible-control"
        type="submit"
        disabled={!ready || sending || active || !draft.trim()}
        aria-label={continuing ? copy.home.followUpSend : copy.home.commandSend}
      >
        {sending || active ? (
          <span className="button-spinner" aria-hidden="true" />
        ) : (
          <Send aria-hidden="true" />
        )}
      </button>
    </form>
  );

  function startNewRequest() {
    setDraft("");
    setSubmitted(false);
    setSubmittedRequest("");
    setError(undefined);
    onStartNew();
    onFocusChange(false);
    if (focusFrameRef.current !== undefined) {
      cancelAnimationFrame(focusFrameRef.current);
    }
    focusFrameRef.current = requestAnimationFrame(() => {
      focusFrameRef.current = undefined;
      inputRef.current?.focus();
    });
  }

  return (
    <section
      className="home-command"
      data-stage={stage}
      aria-labelledby="home-command-title"
    >
      <div className="home-command__heading">
        <div>
          <h2 id="home-command-title">
            {continuing ? copy.home.followUpTitle : copy.home.commandTitle}
          </h2>
          <p>
            {continuing
              ? copy.home.followUpDescription
              : copy.home.commandDescription}
          </p>
        </div>
        <span aria-hidden="true">
          <Sparkles />
        </span>
      </div>
      {!continuing && composer}
      {focused && submittedRequest && (
        <div
          className="home-command__request"
          role="group"
          aria-label={copy.home.commandRequestLabel}
        >
          <span>{copy.home.commandRequestLabel}</span>
          <p>{submittedRequest}</p>
        </div>
      )}
      {error && (
        <p className="assistant-inline-alert" role="alert">
          {error}
        </p>
      )}
      {presentation ? (
        <AssistantInteractiveCanvas
          key={message?.id}
          presentation={presentation}
          onContinue={() => inputRef.current?.focus()}
          onOpenTask={onOpenTask}
          onOpenProject={onOpenProject}
          onOpenSchedule={onOpenSchedule}
        />
      ) : status ? (
        <div className="home-command__result" role="status" aria-live="polite">
          <div>
            <strong>{status.title}</strong>
            <p>{status.description}</p>
          </div>
          {status.needsReview && (
            <button
              className="secondary-button focus-visible-control"
              type="button"
              onClick={() => {
                if (job?.state === "waiting_approval") {
                  onOpenAssistant();
                  return;
                }
                inputRef.current?.focus();
              }}
            >
              {job?.state === "waiting_approval"
                ? copy.home.commandReview
                : copy.home.followUpAction}
            </button>
          )}
        </div>
      ) : null}
      {continuing && (
        <section
          className="home-command__continuation"
          aria-labelledby="home-command-continuation-title"
        >
          <div className="home-command__continuation-heading">
            <span aria-hidden="true">
              <MessageCircleMore />
            </span>
            <div>
              <h3 id="home-command-continuation-title">
                {copy.home.followUpAction}
              </h3>
              <p>{copy.home.followUpContext}</p>
            </div>
            <button
              className="text-button focus-visible-control"
              type="button"
              onClick={startNewRequest}
              disabled={sending || active}
            >
              {copy.home.startNewRequest}
            </button>
          </div>
          {composer}
        </section>
      )}
    </section>
  );
}

type AssistantRailProps = {
  assistantReady: boolean;
  onOpenAssistant(): void;
};

export function AssistantRail({
  assistantReady,
  onOpenAssistant,
}: AssistantRailProps) {
  return (
    <div className="assistant-rail">
      <div className="assistant-rail__identity">
        <span aria-hidden="true">
          <Sparkles />
        </span>
        <div>
          <strong>{copy.home.assistantRailTitle}</strong>
          <p>
            {assistantReady
              ? copy.home.assistantReady
              : copy.home.assistantNeedsConnection}
          </p>
        </div>
      </div>
      <p className="assistant-rail__message">
        {assistantReady
          ? "오늘의 일정과 할 일을 바탕으로 필요한 일을 같이 정리할게요."
          : "ChatGPT를 연결하면 대화를 바로 시작할 수 있어요."}
      </p>
      <div className="assistant-rail__quick-actions">
        <button
          className="focus-visible-control"
          type="button"
          onClick={onOpenAssistant}
        >
          오늘 일정 정리하기
        </button>
        <button
          className="focus-visible-control"
          type="button"
          onClick={onOpenAssistant}
        >
          해야 할 일 정하기
        </button>
      </div>
      <button
        className="assistant-rail__composer focus-visible-control"
        type="button"
        onClick={onOpenAssistant}
      >
        <span>{copy.home.assistantPrompt}</span>
        <MessageCircleMore aria-hidden="true" />
      </button>
    </div>
  );
}

export function EmptySurface({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <div className="empty-surface">
      <Clock3 aria-hidden="true" />
      <div>
        <strong>{title}</strong>
        <p>{description}</p>
      </div>
    </div>
  );
}

function HomeBriefingSkeleton({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="home-briefing-skeleton"
      label={copy.home.loadingDescription}
      visible={visible}
    >
      <SkeletonBlock className="skeleton--briefing-icon" />
      <span className="skeleton-copy-stack">
        <SkeletonBlock className="skeleton--title" />
        <SkeletonBlock className="skeleton--caption" />
      </span>
      <SkeletonBlock className="skeleton--chevron" />
    </SkeletonGroup>
  );
}

function ScheduleSkeleton({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="schedule-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      <SkeletonBlock className="skeleton--schedule-icon" />
      <span className="skeleton-copy-stack">
        <SkeletonBlock className="skeleton--title" />
        <SkeletonBlock className="skeleton--caption" />
      </span>
      <SkeletonBlock className="skeleton--chevron" />
    </SkeletonGroup>
  );
}

function TaskListSkeleton({
  rows,
  visible,
}: {
  rows: number;
  visible: boolean;
}) {
  return (
    <SkeletonGroup
      className="task-list-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      {Array.from({ length: rows }, (_, index) => (
        <span className="task-row-skeleton" key={index}>
          <SkeletonBlock className="skeleton--task-control" />
          <SkeletonBlock className="skeleton--task-title" />
          <SkeletonBlock className="skeleton--task-date" />
        </span>
      ))}
    </SkeletonGroup>
  );
}

function ScheduleHighlight({
  entry,
  onEdit,
}: {
  entry: ScheduleEntry;
  onEdit?: () => void;
}) {
  return (
    <div className="schedule-highlight">
      <span className="schedule-highlight__icon" aria-hidden="true">
        <CalendarDays />
      </span>
      <div>
        <strong>{entry.title}</strong>
        <p>{scheduleDetail(entry)}</p>
      </div>
      {onEdit ? (
        <button
          className="schedule-highlight__edit focus-visible-control"
          type="button"
          onClick={onEdit}
          aria-label={copy.schedule.editSchedule(entry.title)}
        >
          <Pencil aria-hidden="true" />
          <span>{copy.actions.edit}</span>
        </button>
      ) : (
        <span className="schedule-highlight__source">
          {copy.schedule.connectedCalendar}
        </span>
      )}
    </div>
  );
}

function DeadlineBrief({
  tasks,
  onEditTask,
  onOpenTask,
}: {
  tasks: Task[];
  onEditTask(task: Task): void;
  onOpenTask(task: Task): void | Promise<void>;
}) {
  const overdueCount = tasks.filter(
    (task) => taskDueState(task) === "overdue",
  ).length;
  const upcomingCount = tasks.length - overdueCount;
  return (
    <section
      className="home-deadline-brief"
      aria-labelledby="home-deadline-title"
    >
      <header>
        <span className="home-deadline-brief__icon" aria-hidden="true">
          <AlertTriangle />
        </span>
        <div>
          <h2 id="home-deadline-title">{copy.home.deadlineTitle}</h2>
          <p>{copy.home.deadlineSummary(overdueCount, upcomingCount)}</p>
        </div>
        <strong>{copy.home.deadlineCount(tasks.length)}</strong>
      </header>
      <ul>
        {tasks.slice(0, 4).map((task) => {
          const state = taskDueState(task);
          return (
            <li key={task.id} data-due-state={state}>
              <button
                className="home-deadline-brief__task focus-visible-control"
                type="button"
                onClick={() => void onOpenTask(task)}
              >
                <span>{dueStateLabel(state)}</span>
                <strong>{task.title}</strong>
                {task.dueAt && (
                  <time dateTime={task.dueAt}>{formatDueTime(task.dueAt)}</time>
                )}
              </button>
              <button
                className="home-deadline-brief__edit focus-visible-control"
                type="button"
                onClick={() => onEditTask(task)}
                aria-label={copy.home.editTask(task.title)}
              >
                <Pencil aria-hidden="true" />
                <span>{copy.actions.edit}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function greetingForHour(hour: number): string {
  if (hour < 12) return copy.home.morningGreeting;
  if (hour < 18) return copy.home.afternoonGreeting;
  return copy.home.eveningGreeting;
}

function homeAssistantState(
  focused: boolean,
  job: AgentJob | undefined,
  message: ConversationMessage | undefined,
): "overview" | "working" | "result" | "attention" {
  if (!focused) return "overview";
  if (job?.state === "completed" && message?.status === "completed") {
    return "result";
  }
  if (
    job?.state === "failed" ||
    job?.state === "cancelled" ||
    job?.state === "declined"
  ) {
    return "attention";
  }
  return "working";
}

function briefingHeading(
  nextSchedule: ScheduleEntry | undefined,
  scheduleCount: number,
): string {
  if (nextSchedule) return copy.home.briefingWithNext(nextSchedule.title);
  if (scheduleCount) return copy.home.briefingWithSchedule(scheduleCount);
  return copy.home.briefingEmpty;
}

function briefingSummary(scheduleCount: number, taskCount: number): string {
  const parts = [`일정 ${scheduleCount}개`, `할 일 ${taskCount}개`];
  return parts.join(" · ");
}

function scheduleDetail(entry: ScheduleEntry): string {
  const time = `${formatTime(entry.startsAt)} · ${formatTime(entry.endsAt)}`;
  return entry.notes ? `${time} · ${entry.notes}` : time;
}

function relativeScheduleTime(value: string): string {
  const difference = new Date(value).getTime() - Date.now();
  const minutes = Math.round(difference / 60_000);
  if (minutes > 0 && minutes < 60) return `${minutes}분 뒤`;
  return formatTime(value);
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

function dueLabel(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
  }).format(new Date(value));
}

function taskDueLabel(task: Task): string {
  const state = taskDueState(task);
  return state === "later" ? dueLabel(task.dueAt ?? "") : dueStateLabel(state);
}

function dueStateLabel(state: TaskDueState): string {
  if (state === "overdue") return copy.home.overdue;
  if (state === "today") return copy.home.dueToday;
  if (state === "tomorrow") return copy.home.dueTomorrow;
  return "";
}

function formatDueTime(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function isTerminalJob(state: AgentJob["state"]): boolean {
  return ["completed", "failed", "cancelled", "declined"].includes(state);
}

function commandStatus(
  job: AgentJob | undefined,
  message: ConversationMessage | undefined,
): { title: string; description: string; needsReview: boolean } | undefined {
  if (!job) return undefined;
  if (job.state === "waiting_approval") {
    return {
      title: copy.home.commandNeedsReview,
      description: copy.home.commandNeedsReviewDescription,
      needsReview: true,
    };
  }
  if (["failed", "cancelled", "declined"].includes(job.state)) {
    return {
      title: copy.home.commandFailedTitle,
      description: copy.home.commandFailed,
      needsReview: true,
    };
  }
  if (job.state === "completed") {
    return {
      title: copy.home.commandCompleted,
      description: message?.content || copy.home.commandCompletedDescription,
      needsReview: Boolean(message?.content),
    };
  }
  return {
    title: copy.home.commandProcessing,
    description: copy.home.commandProcessingDescription,
    needsReview: false,
  };
}
