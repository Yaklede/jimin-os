import { CalendarDays, CheckCircle2, Circle } from "lucide-react";
import { useState } from "react";

import { type HomeSnapshot } from "../api/home";
import { type ScheduleEntry, type Task } from "../api/planning";
import { copy } from "../copy";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";
import { EmptySurface } from "./HomeWorkspace";

type PlanningWorkspaceProps = {
  snapshot: HomeSnapshot | undefined;
  loading: boolean;
  error: string | undefined;
  onCompleteTask(task: Task): Promise<void>;
};

export function PlanningWorkspace({
  snapshot,
  loading,
  error,
  onCompleteTask,
}: PlanningWorkspaceProps) {
  const [completingTaskId, setCompletingTaskId] = useState<string>();
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;

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
            <h2 id="planning-schedule-title">오늘 일정</h2>
          </div>
          <span>
            {showingSkeleton ? (
              <CountSkeleton visible={skeletonVisible} />
            ) : (
              copy.home.scheduleCount(snapshot?.schedule.length ?? 0)
            )}
          </span>
        </div>
        <div className="planning-surface">
          {showingSkeleton ? (
            <ScheduleTimelineSkeleton rows={3} visible={skeletonVisible} />
          ) : snapshot?.schedule.length ? (
            <ol className="planning-timeline">
              {snapshot.schedule.map((entry) => (
                <ScheduleRow entry={entry} key={entry.id} />
              ))}
            </ol>
          ) : (
            <EmptySurface
              title={copy.home.scheduleEmptyTitle}
              description={copy.schedule.empty}
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
                <li key={task.id}>
                  <button
                    className="planning-task-list__complete focus-visible-control"
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
                  <div>
                    <strong>{task.title}</strong>
                    {task.notes && <p>{task.notes}</p>}
                  </div>
                  {task.dueAt && (
                    <time dateTime={task.dueAt}>{dueLabel(task.dueAt)}</time>
                  )}
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

function ScheduleRow({ entry }: { entry: ScheduleEntry }) {
  return (
    <li>
      <time dateTime={entry.startsAt}>{formatTime(entry.startsAt)}</time>
      <span aria-hidden="true" />
      <div>
        <strong>{entry.title}</strong>
        <p>
          {entry.notes ||
            `${formatTime(entry.startsAt)}–${formatTime(entry.endsAt)}`}
        </p>
      </div>
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

function dueLabel(value: string) {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
  }).format(new Date(value));
}
