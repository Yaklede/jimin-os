import {
  ArrowLeft,
  ArrowRight,
  BriefcaseBusiness,
  CalendarDays,
  ChevronRight,
  Circle,
  Clock3,
  FolderKanban,
  ListTodo,
  MessageCircleMore,
  Mic,
  Send,
  Sparkles,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";

import { type AgentJob, type ConversationMessage } from "../api/agent";
import { type HomeSnapshot } from "../api/home";
import { type ScheduleEntry, type Task } from "../api/planning";
import { type Project } from "../api/projects";
import {
  deriveAssistantPresentation,
  type AssistantPresentation,
} from "../assistantPresentation";
import { copy } from "../copy";
import { createUuidV7 } from "../uuid";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";

type HomeWorkspaceProps = {
  snapshot: HomeSnapshot | undefined;
  loading: boolean;
  error: string | undefined;
  assistantReady: boolean;
  assistantJob: AgentJob | undefined;
  assistantMessage: ConversationMessage | undefined;
  projects: Project[];
  onOpenAssistant(): void;
  onSendAssistant(text: string, clientMessageId: string): Promise<boolean>;
  onCompleteTask(task: Task): Promise<void>;
  onOpenTask(task: Task): void;
  onOpenProject(project: Project): void;
  onOpenSchedule(): void;
};

export function HomeWorkspace({
  snapshot,
  loading,
  error,
  assistantReady,
  assistantJob,
  assistantMessage,
  projects,
  onOpenAssistant,
  onSendAssistant,
  onCompleteTask,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
}: HomeWorkspaceProps) {
  const [completingTaskId, setCompletingTaskId] = useState<string>();
  const [assistantFocused, setAssistantFocused] = useState(false);
  const [highlightedHomeTaskId, setHighlightedHomeTaskId] = useState<string>();
  const highlightedHomeTaskRef = useRef<HTMLLIElement | null>(null);
  const greeting = useMemo(() => greetingForHour(new Date().getHours()), []);
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;
  const nextSchedule = snapshot?.schedule[0];
  const scheduleCount = snapshot?.schedule.length ?? 0;
  const taskCount = snapshot?.tasks.length ?? 0;

  useEffect(() => {
    if (!highlightedHomeTaskId || assistantFocused) return;
    const element = highlightedHomeTaskRef.current;
    if (!element) return;
    element.scrollIntoView({ block: "center", behavior: "smooth" });
    element.focus({ preventScroll: true });
  }, [assistantFocused, highlightedHomeTaskId]);

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
    <section className="home-page" aria-busy={showingSkeleton}>
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

      <HomeAssistantCommand
        ready={assistantReady}
        job={assistantJob}
        message={assistantMessage}
        snapshot={snapshot}
        projects={projects}
        focused={assistantFocused}
        onFocusChange={setAssistantFocused}
        onOpenAssistant={onOpenAssistant}
        onSend={onSendAssistant}
        onOpenTask={(task) => {
          if (task.projectId) {
            onOpenTask(task);
            return;
          }
          setHighlightedHomeTaskId(task.id);
          setAssistantFocused(false);
        }}
        onOpenProject={onOpenProject}
        onOpenSchedule={onOpenSchedule}
      />

      {!assistantFocused && (
        <>
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
              <ScheduleHighlight entry={nextSchedule} />
            ) : (
              <EmptySurface
                title={copy.home.scheduleEmptyTitle}
                description={copy.home.scheduleEmptyDescription}
              />
            )}
          </section>

          <section className="home-tasks" aria-labelledby="today-task-title">
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
                        <time dateTime={task.dueAt}>
                          {dueLabel(task.dueAt)}
                        </time>
                      )}
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
  job,
  message,
  snapshot,
  projects,
  focused,
  onFocusChange,
  onOpenAssistant,
  onSend,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
}: {
  ready: boolean;
  job: AgentJob | undefined;
  message: ConversationMessage | undefined;
  snapshot: HomeSnapshot | undefined;
  projects: Project[];
  focused: boolean;
  onFocusChange(focused: boolean): void;
  onOpenAssistant(): void;
  onSend(text: string, clientMessageId: string): Promise<boolean>;
  onOpenTask(task: Task): void;
  onOpenProject(project: Project): void;
  onOpenSchedule(): void;
}) {
  const [draft, setDraft] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const [lastRequest, setLastRequest] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string>();
  const active = Boolean(job && !isTerminalJob(job.state));

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const request = draft.trim();
    if (!request || sending || active || !ready) return;
    setSending(true);
    setError(undefined);
    const sent = await onSend(request, createUuidV7());
    if (sent) {
      setDraft("");
      setSubmitted(true);
      setLastRequest(request);
      onFocusChange(true);
    } else {
      setError(copy.home.commandFailed);
    }
    setSending(false);
  }

  const status = submitted && focused ? commandStatus(job, message) : undefined;
  const presentation =
    focused && submitted && job?.state === "completed" && message
      ? deriveAssistantPresentation(
          lastRequest,
          message.content,
          snapshot,
          projects,
        )
      : undefined;

  return (
    <section className="home-command" aria-labelledby="home-command-title">
      <div className="home-command__heading">
        <div>
          <h2 id="home-command-title">{copy.home.commandTitle}</h2>
          <p>{copy.home.commandDescription}</p>
        </div>
        <span aria-hidden="true">
          <Sparkles />
        </span>
      </div>
      <form
        className="home-command__form"
        aria-busy={sending || active}
        onSubmit={(event) => void submit(event)}
      >
        <label className="sr-only" htmlFor="home-assistant-command">
          {copy.home.commandLabel}
        </label>
        <input
          id="home-assistant-command"
          value={draft}
          maxLength={24_000}
          placeholder={
            ready
              ? copy.home.commandInputPlaceholder
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
          aria-label={copy.home.commandSend}
        >
          {sending || active ? (
            <span className="button-spinner" aria-hidden="true" />
          ) : (
            <Send aria-hidden="true" />
          )}
        </button>
      </form>
      {error && (
        <p className="assistant-inline-alert" role="alert">
          {error}
        </p>
      )}
      {presentation ? (
        <AdaptiveAssistantResult
          presentation={presentation}
          projects={projects}
          onOpenAssistant={onOpenAssistant}
          onOpenTask={onOpenTask}
          onOpenProject={onOpenProject}
          onOpenSchedule={onOpenSchedule}
          onReset={() => onFocusChange(false)}
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
              onClick={onOpenAssistant}
            >
              {copy.home.commandReview}
            </button>
          )}
        </div>
      ) : null}
    </section>
  );
}

function AdaptiveAssistantResult({
  presentation,
  projects,
  onOpenAssistant,
  onOpenTask,
  onOpenProject,
  onOpenSchedule,
  onReset,
}: {
  presentation: AssistantPresentation;
  projects: Project[];
  onOpenAssistant(): void;
  onOpenTask(task: Task): void;
  onOpenProject(project: Project): void;
  onOpenSchedule(): void;
  onReset(): void;
}) {
  return (
    <section
      className="assistant-result"
      aria-labelledby="assistant-result-title"
    >
      <header className="assistant-result__header">
        <div>
          <p>{copy.home.resultEyebrow}</p>
          <h3 id="assistant-result-title">{presentation.title}</h3>
        </div>
        <button
          className="text-button focus-visible-control"
          type="button"
          onClick={onReset}
        >
          <ArrowLeft aria-hidden="true" />
          {copy.home.backToBriefing}
        </button>
      </header>
      <p className="assistant-result__summary" aria-live="polite">
        {presentation.summary}
      </p>

      {presentation.kind === "tasks" && (
        <AssistantTaskResult
          presentation={presentation}
          projects={projects}
          onOpenTask={onOpenTask}
        />
      )}
      {presentation.kind === "schedule" && (
        <AssistantScheduleResult
          presentation={presentation}
          onOpenSchedule={onOpenSchedule}
        />
      )}
      {presentation.kind === "projects" && (
        <AssistantProjectResult
          presentation={presentation}
          onOpenProject={onOpenProject}
        />
      )}
      {presentation.kind === "summary" && (
        <button
          className="secondary-button assistant-result__follow-up focus-visible-control"
          type="button"
          onClick={onOpenAssistant}
        >
          {copy.home.continueRequest}
          <ArrowRight aria-hidden="true" />
        </button>
      )}
    </section>
  );
}

function AssistantTaskResult({
  presentation,
  projects,
  onOpenTask,
}: {
  presentation: Extract<AssistantPresentation, { kind: "tasks" }>;
  projects: Project[];
  onOpenTask(task: Task): void;
}) {
  if (!presentation.items.length) {
    return (
      <ResultEmpty
        icon={<ListTodo aria-hidden="true" />}
        description={copy.home.noMatchingTasks}
      />
    );
  }
  return (
    <ul className="assistant-result-list assistant-result-list--tasks">
      {presentation.items.map((task) => {
        const project = projects.find((item) => item.id === task.projectId);
        return (
          <li
            key={task.id}
            data-highlighted={task.id === presentation.highlightedTaskId}
          >
            <button
              className="assistant-result-row focus-visible-control"
              type="button"
              onClick={() => onOpenTask(task)}
            >
              <span className="assistant-result-row__icon" aria-hidden="true">
                <ListTodo />
              </span>
              <span className="assistant-result-row__main">
                <strong>{task.title}</strong>
                <span>{project?.title || copy.home.unassignedTask}</span>
              </span>
              <span className="assistant-result-row__meta">
                {task.dueAt && (
                  <time dateTime={task.dueAt}>{dueLabel(task.dueAt)}</time>
                )}
                <span>{copy.home.openTaskAction}</span>
                <ChevronRight aria-hidden="true" />
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function AssistantScheduleResult({
  presentation,
  onOpenSchedule,
}: {
  presentation: Extract<AssistantPresentation, { kind: "schedule" }>;
  onOpenSchedule(): void;
}) {
  if (!presentation.items.length) {
    return (
      <ResultEmpty
        icon={<CalendarDays aria-hidden="true" />}
        description={copy.home.noScheduleResult}
      />
    );
  }
  return (
    <ul className="assistant-result-list">
      {presentation.items.map((entry) => (
        <li key={entry.id}>
          <button
            className="assistant-result-row focus-visible-control"
            type="button"
            onClick={onOpenSchedule}
          >
            <span className="assistant-result-row__icon" aria-hidden="true">
              <CalendarDays />
            </span>
            <span className="assistant-result-row__main">
              <strong>{entry.title}</strong>
              <span>{scheduleDetail(entry)}</span>
            </span>
            <span className="assistant-result-row__meta">
              <span>{copy.home.openScheduleAction}</span>
              <ChevronRight aria-hidden="true" />
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}

function AssistantProjectResult({
  presentation,
  onOpenProject,
}: {
  presentation: Extract<AssistantPresentation, { kind: "projects" }>;
  onOpenProject(project: Project): void;
}) {
  if (!presentation.items.length) {
    return (
      <ResultEmpty
        icon={<FolderKanban aria-hidden="true" />}
        description={copy.home.noMatchingProjects}
      />
    );
  }
  return (
    <ul className="assistant-result-list">
      {presentation.items.map((project) => (
        <li key={project.id}>
          <button
            className="assistant-result-row focus-visible-control"
            type="button"
            onClick={() => onOpenProject(project)}
          >
            <span className="assistant-result-row__icon" aria-hidden="true">
              <BriefcaseBusiness />
            </span>
            <span className="assistant-result-row__main">
              <strong>{project.title}</strong>
              <span>
                {project.nextAction ||
                  project.objective ||
                  copy.projects.noNextAction}
              </span>
            </span>
            <span className="assistant-result-row__meta">
              <span>{copy.home.openProjectAction}</span>
              <ChevronRight aria-hidden="true" />
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}

function ResultEmpty({
  icon,
  description,
}: {
  icon: React.ReactNode;
  description: string;
}) {
  return (
    <div className="assistant-result__empty">
      {icon}
      <p>{description}</p>
    </div>
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

function ScheduleHighlight({ entry }: { entry: ScheduleEntry }) {
  return (
    <div className="schedule-highlight">
      <span className="schedule-highlight__icon" aria-hidden="true">
        <CalendarDays />
      </span>
      <div>
        <strong>{entry.title}</strong>
        <p>{scheduleDetail(entry)}</p>
      </div>
      <ChevronRight aria-hidden="true" />
    </div>
  );
}

function greetingForHour(hour: number): string {
  if (hour < 12) return copy.home.morningGreeting;
  if (hour < 18) return copy.home.afternoonGreeting;
  return copy.home.eveningGreeting;
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
