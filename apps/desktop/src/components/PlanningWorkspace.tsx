import {
  CalendarClock,
  CalendarDays,
  CheckCircle2,
  Circle,
  History,
  Pencil,
  RotateCcw,
} from "lucide-react";
import { useEffect, useRef, useState, type RefObject } from "react";

import {
  type PlanningSnapshot,
  type ScheduleEntry,
  type Task,
} from "../api/planning";
import { copy } from "../copy";
import { taskDueState } from "../planningDue";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";
import { EmptySurface } from "./HomeWorkspace";

type PlanningWorkspaceProps = {
  snapshot: PlanningSnapshot | undefined;
  loading: boolean;
  error: string | undefined;
  highlightedScheduleId?: string;
  highlightedTaskId?: string;
  onCompleteTask(task: Task): Promise<void>;
  onRestoreTask(task: Task): Promise<void>;
  onEditTask(task: Task): void;
  onEditSchedule(entry: ScheduleEntry): void;
};

export function PlanningWorkspace({
  snapshot,
  loading,
  error,
  highlightedScheduleId,
  highlightedTaskId,
  onCompleteTask,
  onRestoreTask,
  onEditTask,
  onEditSchedule,
}: PlanningWorkspaceProps) {
  const [pendingTask, setPendingTask] = useState<{
    id: string;
    action: "complete" | "restore";
  }>();
  const highlightedScheduleRef = useRef<HTMLLIElement | null>(null);
  const highlightedTaskRef = useRef<HTMLLIElement | null>(null);
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;
  const now = Date.now();
  const upcomingSchedule =
    snapshot?.schedule.filter(
      (entry) => new Date(entry.endsAt).getTime() >= now,
    ) ?? [];
  const pastSchedule = [
    ...(snapshot?.schedule.filter(
      (entry) => new Date(entry.endsAt).getTime() < now,
    ) ?? []),
  ].reverse();

  useEffect(() => {
    if (!highlightedScheduleId) return;
    const element = highlightedScheduleRef.current;
    if (!element) return;
    element.scrollIntoView({
      block: "center",
      behavior: preferredScrollBehavior(),
    });
    element.focus({ preventScroll: true });
  }, [highlightedScheduleId, snapshot?.schedule]);

  useEffect(() => {
    if (!highlightedTaskId) return;
    const element = highlightedTaskRef.current;
    if (!element) return;
    element.scrollIntoView({
      block: "center",
      behavior: preferredScrollBehavior(),
    });
    element.focus({ preventScroll: true });
  }, [highlightedTaskId, snapshot?.tasks]);

  async function complete(task: Task) {
    if (pendingTask) return;
    setPendingTask({ id: task.id, action: "complete" });
    try {
      await onCompleteTask(task);
    } finally {
      setPendingTask(undefined);
    }
  }

  async function restore(task: Task) {
    if (pendingTask) return;
    setPendingTask({ id: task.id, action: "restore" });
    try {
      await onRestoreTask(task);
    } finally {
      setPendingTask(undefined);
    }
  }

  return (
    <section className="planning-page" aria-busy={showingSkeleton}>
      <header className="page-heading">
        <p>{todayLabel()}</p>
        <h1>{copy.schedule.title}</h1>
        <span>{copy.schedule.description}</span>
      </header>
      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}

      <section
        className="planning-schedule"
        aria-labelledby="planning-schedule-title"
      >
        <div className="planning-section-heading">
          <div>
            <CalendarDays aria-hidden="true" />
            <h2 id="planning-schedule-title">{copy.schedule.upcomingTitle}</h2>
          </div>
          <span>
            {showingSkeleton ? (
              <CountSkeleton visible={skeletonVisible} />
            ) : (
              copy.home.scheduleCount(upcomingSchedule.length)
            )}
          </span>
        </div>
        <div className="planning-surface">
          {showingSkeleton ? (
            <ScheduleTimelineSkeleton rows={3} visible={skeletonVisible} />
          ) : upcomingSchedule.length ? (
            <ol className="planning-timeline">
              {upcomingSchedule.map((entry) => (
                <ScheduleRow
                  entry={entry}
                  highlighted={entry.id === highlightedScheduleId}
                  elementRef={
                    entry.id === highlightedScheduleId
                      ? highlightedScheduleRef
                      : undefined
                  }
                  onEdit={onEditSchedule}
                  key={entry.id}
                />
              ))}
            </ol>
          ) : (
            <EmptySurface
              title={copy.home.scheduleEmptyTitle}
              description={copy.schedule.upcomingEmpty}
            />
          )}
        </div>
      </section>

      <section className="planning-tasks" aria-labelledby="planning-task-title">
        <div className="planning-section-heading">
          <div>
            <CheckCircle2 aria-hidden="true" />
            <h2 id="planning-task-title">{copy.tasks.title}</h2>
          </div>
          <span>
            {showingSkeleton ? (
              <CountSkeleton visible={skeletonVisible} />
            ) : (
              copy.home.taskCount(snapshot?.tasks.length ?? 0)
            )}
          </span>
        </div>
        <div className="planning-surface">
          {showingSkeleton ? (
            <PlanningTaskSkeleton rows={4} visible={skeletonVisible} />
          ) : snapshot?.tasks.length ? (
            <ul className="planning-task-list">
              {snapshot.tasks.map((task) => (
                <li
                  key={task.id}
                  ref={
                    task.id === highlightedTaskId
                      ? highlightedTaskRef
                      : undefined
                  }
                  data-highlighted={task.id === highlightedTaskId}
                  tabIndex={task.id === highlightedTaskId ? -1 : undefined}
                >
                  <button
                    className="planning-task-list__complete focus-visible-control"
                    type="button"
                    onClick={() => void complete(task)}
                    disabled={Boolean(pendingTask)}
                    aria-label={copy.home.completeTask(task.title)}
                  >
                    {pendingTask?.id === task.id &&
                    pendingTask.action === "complete" ? (
                      <span className="button-spinner" aria-hidden="true" />
                    ) : (
                      <Circle aria-hidden="true" />
                    )}
                  </button>
                  <div>
                    <strong>{task.title}</strong>
                    {task.notes && <p>{task.notes}</p>}
                  </div>
                  {task.dueAt && (
                    <time
                      dateTime={task.dueAt}
                      data-due-state={taskDueState(task)}
                    >
                      {taskDueLabel(task)}
                    </time>
                  )}
                  <button
                    className="planning-row-edit focus-visible-control"
                    type="button"
                    onClick={() => onEditTask(task)}
                    disabled={Boolean(pendingTask)}
                    aria-label={copy.home.editTask(task.title)}
                  >
                    <Pencil aria-hidden="true" />
                    <span>{copy.actions.edit}</span>
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <EmptySurface
              title={copy.home.taskEmptyTitle}
              description={copy.tasks.empty}
            />
          )}
        </div>
      </section>

      <section
        className="planning-history"
        aria-labelledby="planning-history-title"
      >
        <div className="planning-section-heading">
          <div>
            <CalendarClock aria-hidden="true" />
            <h2 id="planning-history-title">{copy.schedule.historyTitle}</h2>
          </div>
          <span>
            {showingSkeleton ? (
              <CountSkeleton visible={skeletonVisible} />
            ) : (
              copy.home.scheduleCount(pastSchedule.length)
            )}
          </span>
        </div>
        <p className="planning-section-description">
          {copy.schedule.historyDescription}
        </p>
        <div className="planning-surface">
          {showingSkeleton ? (
            <ScheduleTimelineSkeleton rows={2} visible={skeletonVisible} />
          ) : pastSchedule.length ? (
            <ol className="planning-timeline planning-timeline--history">
              {pastSchedule.map((entry) => (
                <ScheduleRow
                  entry={entry}
                  highlighted={entry.id === highlightedScheduleId}
                  elementRef={
                    entry.id === highlightedScheduleId
                      ? highlightedScheduleRef
                      : undefined
                  }
                  onEdit={onEditSchedule}
                  key={entry.id}
                />
              ))}
            </ol>
          ) : (
            <EmptySurface
              title={copy.schedule.historyTitle}
              description={copy.schedule.historyEmpty}
            />
          )}
        </div>
      </section>

      <section
        className="planning-completed"
        aria-labelledby="planning-completed-title"
      >
        <div className="planning-section-heading">
          <div>
            <History aria-hidden="true" />
            <h2 id="planning-completed-title">{copy.tasks.completedTitle}</h2>
          </div>
          <span>
            {showingSkeleton ? (
              <CountSkeleton visible={skeletonVisible} />
            ) : (
              copy.home.taskCount(snapshot?.completedTasks.length ?? 0)
            )}
          </span>
        </div>
        <div className="planning-surface">
          {showingSkeleton ? (
            <PlanningTaskSkeleton rows={2} visible={skeletonVisible} />
          ) : snapshot?.completedTasks.length ? (
            <ul className="planning-task-list planning-task-list--completed">
              {snapshot.completedTasks.map((task) => (
                <li key={task.id}>
                  <button
                    className="planning-task-list__restore focus-visible-control"
                    type="button"
                    onClick={() => void restore(task)}
                    disabled={Boolean(pendingTask)}
                    aria-label={copy.tasks.restoreTask(task.title)}
                  >
                    {pendingTask?.id === task.id &&
                    pendingTask.action === "restore" ? (
                      <span className="button-spinner" aria-hidden="true" />
                    ) : (
                      <RotateCcw aria-hidden="true" />
                    )}
                  </button>
                  <div>
                    <strong>{task.title}</strong>
                    {task.notes && <p>{task.notes}</p>}
                  </div>
                  {task.completedAt && (
                    <time dateTime={task.completedAt}>
                      {copy.tasks.completedAt(
                        completedAtLabel(task.completedAt),
                      )}
                    </time>
                  )}
                </li>
              ))}
            </ul>
          ) : (
            <EmptySurface
              title={copy.tasks.completedEmptyTitle}
              description={copy.tasks.completedEmptyDescription}
            />
          )}
        </div>
      </section>
    </section>
  );
}

function CountSkeleton({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="skeleton-count"
      label={copy.home.loadingShort}
      visible={visible}
    >
      <SkeletonBlock />
    </SkeletonGroup>
  );
}

function ScheduleTimelineSkeleton({
  rows,
  visible,
}: {
  rows: number;
  visible: boolean;
}) {
  return (
    <SkeletonGroup
      className="planning-timeline-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      {Array.from({ length: rows }, (_, index) => (
        <span className="planning-timeline-skeleton__row" key={index}>
          <SkeletonBlock className="skeleton--timeline-time" />
          <SkeletonBlock className="skeleton--timeline-dot" />
          <span className="skeleton-copy-stack">
            <SkeletonBlock className="skeleton--title" />
            <SkeletonBlock className="skeleton--caption" />
          </span>
        </span>
      ))}
    </SkeletonGroup>
  );
}

function PlanningTaskSkeleton({
  rows,
  visible,
}: {
  rows: number;
  visible: boolean;
}) {
  return (
    <SkeletonGroup
      className="planning-task-skeleton"
      label={copy.home.loadingShort}
      visible={visible}
    >
      {Array.from({ length: rows }, (_, index) => (
        <span className="planning-task-skeleton__row" key={index}>
          <SkeletonBlock className="skeleton--task-control" />
          <span className="skeleton-copy-stack">
            <SkeletonBlock className="skeleton--title" />
            <SkeletonBlock className="skeleton--caption" />
          </span>
          <SkeletonBlock className="skeleton--task-date" />
        </span>
      ))}
    </SkeletonGroup>
  );
}

function ScheduleRow({
  entry,
  highlighted,
  elementRef,
  onEdit,
}: {
  entry: ScheduleEntry;
  highlighted: boolean;
  elementRef?: RefObject<HTMLLIElement | null>;
  onEdit(entry: ScheduleEntry): void;
}) {
  return (
    <li
      ref={elementRef}
      data-highlighted={highlighted}
      tabIndex={highlighted ? -1 : undefined}
    >
      <time dateTime={entry.startsAt}>
        <span>{scheduleDayLabel(entry.startsAt)}</span>
        <strong>{formatTime(entry.startsAt)}</strong>
      </time>
      <span aria-hidden="true" />
      <div>
        <strong>{entry.title}</strong>
        <p>
          {entry.notes ||
            `${formatTime(entry.startsAt)}–${formatTime(entry.endsAt)}`}
        </p>
      </div>
      {entry.source === "manual" ? (
        <button
          className="planning-row-edit focus-visible-control"
          type="button"
          onClick={() => onEdit(entry)}
          aria-label={copy.schedule.editSchedule(entry.title)}
        >
          <Pencil aria-hidden="true" />
          <span>{copy.actions.edit}</span>
        </button>
      ) : (
        <span
          className="planning-row-source"
          title={copy.schedule.connectedCalendarEdit}
        >
          {copy.schedule.connectedCalendar}
        </span>
      )}
    </li>
  );
}

function todayLabel() {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    weekday: "long",
  }).format(new Date());
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(value));
}

function scheduleDayLabel(value: string) {
  const date = new Date(value);
  const today = new Date();
  const startOfToday = new Date(
    today.getFullYear(),
    today.getMonth(),
    today.getDate(),
  );
  const startOfDate = new Date(
    date.getFullYear(),
    date.getMonth(),
    date.getDate(),
  );
  const difference = Math.round(
    (startOfDate.getTime() - startOfToday.getTime()) / 86_400_000,
  );
  if (difference === 0) return copy.schedule.todayLabel;
  if (difference === 1) return copy.schedule.tomorrowLabel;
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
    weekday: "short",
  }).format(date);
}

function dueLabel(value: string) {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
  }).format(new Date(value));
}

function taskDueLabel(task: Task): string {
  const state = taskDueState(task);
  if (state === "overdue") return copy.home.overdue;
  if (state === "today") return copy.home.dueToday;
  if (state === "tomorrow") return copy.home.dueTomorrow;
  return task.dueAt ? dueLabel(task.dueAt) : "";
}

function completedAtLabel(value: string) {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function preferredScrollBehavior(): ScrollBehavior {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ? "auto"
    : "smooth";
}
