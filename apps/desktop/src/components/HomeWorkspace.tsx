import {
  CalendarDays,
  ChevronRight,
  Circle,
  Clock3,
  MessageCircleMore,
  Mic,
  Sparkles,
} from "lucide-react";
import { useMemo, useState } from "react";

import { type HomeSnapshot } from "../api/home";
import { type ScheduleEntry, type Task } from "../api/planning";
import { copy } from "../copy";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";

type HomeWorkspaceProps = {
  snapshot: HomeSnapshot | undefined;
  loading: boolean;
  error: string | undefined;
  onOpenAssistant(): void;
  onCompleteTask(task: Task): Promise<void>;
};

export function HomeWorkspace({
  snapshot,
  loading,
  error,
  onOpenAssistant,
  onCompleteTask,
}: HomeWorkspaceProps) {
  const [completingTaskId, setCompletingTaskId] = useState<string>();
  const greeting = useMemo(() => greetingForHour(new Date().getHours()), []);
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;
  const nextSchedule = snapshot?.schedule[0];
  const scheduleCount = snapshot?.schedule.length ?? 0;
  const taskCount = snapshot?.tasks.length ?? 0;

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
              <strong>{briefingHeading(nextSchedule, scheduleCount)}</strong>
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
                <li key={task.id}>
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
                    <time dateTime={task.dueAt}>{dueLabel(task.dueAt)}</time>
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
