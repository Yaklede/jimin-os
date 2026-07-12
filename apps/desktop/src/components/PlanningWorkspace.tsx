import { CalendarDays, CheckCircle2, Circle } from "lucide-react";
import { useState } from "react";

import { type HomeSnapshot } from "../api/home";
import { type ScheduleEntry, type Task } from "../api/planning";
import { copy } from "../copy";
import { EmptySurface, LoadingRows } from "./HomeWorkspace";

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
    <section className="planning-page" aria-busy={loading}>
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
            {loading
              ? copy.home.loadingShort
              : copy.home.scheduleCount(snapshot?.schedule.length ?? 0)}
          </span>
        </div>
        <div className="planning-surface">
          {loading ? (
            <LoadingRows rows={3} />
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
            {loading
              ? copy.home.loadingShort
              : copy.home.taskCount(snapshot?.tasks.length ?? 0)}
          </span>
        </div>
        <div className="planning-surface">
          {loading ? (
            <LoadingRows rows={4} />
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
