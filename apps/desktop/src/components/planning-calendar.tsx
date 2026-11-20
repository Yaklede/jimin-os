import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import { PlanningWeekCalendar } from "./planning-week-calendar";
import {
  shouldPinCalendarPreview,
  useCalendarTaskPreview,
} from "./CalendarTaskPreview";
import type { ScheduleEntry, Task, PlanningSnapshot } from "../api/planning";
import { copy } from "../copy";
import {
  planningCalendarDays,
  samePlanningDay,
  scheduleOverlapsPlanningDay,
  type PlanningViewRange,
} from "../planningRange";

export interface CalendarReport {
  id: string;
  title: string;
  receivedAt: string;
}

interface PlanningCalendarProps {
  range: PlanningViewRange;
  snapshot: PlanningSnapshot | undefined;
  loading: boolean;
  reports?: CalendarReport[];
  onSelectDate(date: Date): void;
  onOpenSchedule?(entry: ScheduleEntry): void;
  onOpenTask?(task: Task): void;
  onOpenReport?(id: string, date: Date): void;
}

export function PlanningCalendar({
  range,
  snapshot,
  loading,
  onSelectDate,
  reports,
  onOpenSchedule,
  onOpenTask,
  onOpenReport,
}: PlanningCalendarProps) {
  const hintId = useId();
  const buttons = useRef<Array<HTMLButtonElement | null>>([]);
  const pressedWithTouch = useRef(false);
  const preview = useCalendarTaskPreview(
    snapshot?.tasks,
    range.mode === "month" ? range.from.getTime() : -1,
    loading,
    onOpenTask,
  );
  const days = planningCalendarDays(range);
  const restoreDateFocus = useRef(false);
  useEffect(() => {
    if (loading || !restoreDateFocus.current) return;
    const index = planningCalendarDays(range).findIndex((date) =>
      samePlanningDay(date, range.anchor),
    );
    buttons.current[index]?.focus({ preventScroll: true });
    restoreDateFocus.current = false;
  }, [loading, range.anchor.getTime(), range.mode]);
  const today = new Date();
  const dateLabel = new Intl.DateTimeFormat("ko-KR", {
    year: "numeric",
    month: "long",
    day: "numeric",
    weekday: "long",
  });
  const weekdayLabel = new Intl.DateTimeFormat("ko-KR", { weekday: "short" });
  const inRange = (date: Date) => date >= range.from && date < range.to;

  function moveFocus(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const offset = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 }[
      event.key
    ];
    const target =
      event.key === "Home"
        ? index - (index % 7)
        : event.key === "End"
          ? index + 6 - (index % 7)
          : offset !== undefined
            ? index + offset
            : undefined;
    if (target === undefined) return;
    event.preventDefault();
    const direction =
      event.key === "Home"
        ? 1
        : event.key === "End"
          ? -1
          : target >= index
            ? 1
            : -1;
    let candidate = target;
    while (
      candidate >= 0 &&
      candidate < days.length &&
      !inRange(days[candidate])
    )
      candidate += direction;
    if (candidate >= 0 && candidate < days.length)
      buttons.current[candidate]?.focus();
  }

  if (range.mode === "week")
    return (
      <PlanningWeekCalendar
        range={range}
        snapshot={snapshot}
        loading={loading}
        reports={reports}
        onSelectDate={onSelectDate}
        onOpenSchedule={onOpenSchedule}
        onOpenTask={onOpenTask}
        onOpenReport={onOpenReport}
      />
    );

  return (
    <section
      className="planning-calendar"
      data-mode={range.mode}
      aria-label={
        range.mode === "month"
          ? copy.schedule.monthCalendar
          : copy.schedule.weekCalendar
      }
    >
      <p id={hintId} className="sr-only">
        {copy.schedule.calendarKeyboardHint}
      </p>
      <div className="planning-calendar__weekdays" aria-hidden="true">
        {days.slice(0, 7).map((date) => (
          <span key={date.getDay()}>{weekdayLabel.format(date)}</span>
        ))}
      </div>
      <div
        className="planning-calendar__grid"
        role="group"
        aria-describedby={hintId}
        aria-label={copy.schedule.selectCalendarDate}
      >
        {days.map((date, index) => {
          const selected = samePlanningDay(date, range.anchor);
          const current = samePlanningDay(date, today);
          const outside = !inRange(date);
          const tasks =
            snapshot?.tasks.filter(
              (task) =>
                task.dueAt && samePlanningDay(new Date(task.dueAt), date),
            ) ?? [];
          const schedule =
            snapshot?.schedule.filter(
              (entry) =>
                entry.status !== "cancelled" &&
                scheduleOverlapsPlanningDay(entry, date),
            ) ?? [];
          const dayReports = (reports ?? []).filter((report) =>
            samePlanningDay(new Date(report.receivedAt), date),
          );
          const canPreview = !reports && !outside && tasks.length > 0;
          const entries = [
            ...dayReports.map((report) => ({
              id: report.id,
              title: report.title,
              kind: "report",
            })),
            ...schedule.map((entry) => ({
              id: entry.id,
              title: entry.title,
              kind: "schedule",
            })),
            ...tasks.map((task) => ({
              id: task.id,
              title: task.title,
              kind: "task",
            })),
          ];
          return (
            <button
              type="button"
              key={date.getTime()}
              ref={(element) => {
                buttons.current[index] = element;
              }}
              className="planning-calendar__day focus-visible-control"
              data-selected={selected}
              data-today={current}
              data-outside={outside}
              disabled={outside || loading}
              tabIndex={selected ? 0 : -1}
              aria-pressed={selected}
              aria-current={current ? "date" : undefined}
              aria-describedby={
                canPreview && preview.anchor === buttons.current[index]
                  ? preview.descriptionId
                  : undefined
              }
              aria-label={
                reports
                  ? copy.projects.inflowHomeCalendarDate(
                      dateLabel.format(date),
                      dayReports.length,
                    )
                  : copy.schedule.calendarDateLabel(
                      dateLabel.format(date),
                      tasks.length,
                      schedule.length,
                    )
              }
              onPointerOver={
                canPreview
                  ? (event) => {
                      if (event.pointerType !== "mouse") return;
                      const taskId =
                        event.target instanceof Element
                          ? event.target.closest<HTMLElement>("[data-task-id]")
                              ?.dataset.taskId
                          : undefined;
                      const hoveredTask = tasks.find(
                        (task) => task.id === taskId,
                      );
                      preview.open(
                        event.currentTarget,
                        "pointer",
                        hoveredTask ? [hoveredTask] : tasks,
                      );
                    }
                  : undefined
              }
              onPointerLeave={canPreview ? preview.leave : undefined}
              onPointerDown={
                canPreview
                  ? (event) => {
                      pressedWithTouch.current = event.pointerType === "touch";
                    }
                  : undefined
              }
              onPointerCancel={
                canPreview
                  ? () => {
                      pressedWithTouch.current = false;
                    }
                  : undefined
              }
              onFocus={
                canPreview
                  ? (event) => {
                      if (event.currentTarget.matches(":focus-visible"))
                        preview.open(event.currentTarget, "focus", tasks);
                    }
                  : undefined
              }
              onBlur={
                canPreview
                  ? (event) => preview.blur(event.currentTarget)
                  : undefined
              }
              onClick={(event) => {
                const pin =
                  canPreview &&
                  shouldPinCalendarPreview(
                    event.nativeEvent,
                    pressedWithTouch.current,
                  );
                if (pin) preview.open(event.currentTarget, "touch", tasks);
                else preview.close();
                pressedWithTouch.current = false;
                restoreDateFocus.current = !pin;
                onSelectDate(date);
              }}
              onKeyDown={(event) => moveFocus(event, index)}
            >
              <span className="planning-calendar__markers" aria-hidden="true">
                {!outside && dayReports.length > 0 && (
                  <span data-kind="report" />
                )}
                {!outside && tasks.length > 0 && <span data-kind="task" />}
                {!outside && schedule.length > 0 && (
                  <span data-kind="schedule" />
                )}
              </span>
              <span className="planning-calendar__number">
                {date.getDate()}
              </span>
              {reports && (
                <span
                  className="planning-calendar__report-count"
                  data-count={dayReports.length}
                  aria-hidden="true"
                >
                  {!outside && dayReports.length > 0
                    ? copy.projects.inflowHomeCalendarCount(dayReports.length)
                    : ""}
                </span>
              )}
              {!outside && entries.length > 0 && (
                <>
                  <span
                    className="planning-calendar__entries"
                    aria-hidden="true"
                  >
                    {entries.slice(0, 2).map((entry) => (
                      <span
                        className="planning-calendar__entry"
                        data-kind={entry.kind}
                        data-task-id={
                          entry.kind === "task" ? entry.id : undefined
                        }
                        key={`${entry.kind}:${entry.id}`}
                      >
                        {entry.title}
                      </span>
                    ))}
                    {entries.length > 2 && (
                      <span className="planning-calendar__more">
                        {copy.schedule.calendarMore(entries.length - 2)}
                      </span>
                    )}
                  </span>
                </>
              )}
            </button>
          );
        })}
      </div>
      {preview.content}
      <div className="planning-calendar__legend">
        {reports ? (
          <span>
            <i data-kind="report" aria-hidden="true" />
            {copy.projects.inflowHomeQueueTitle}
          </span>
        ) : (
          <>
            <span>
              <i data-kind="task" aria-hidden="true" />
              {copy.schedule.calendarTask}
            </span>
            <span>
              <i data-kind="schedule" aria-hidden="true" />
              {copy.schedule.title}
            </span>
          </>
        )}
      </div>
    </section>
  );
}
