import {
  useEffect,
  useId,
  useRef,
  type CSSProperties,
  type KeyboardEvent,
} from "react";
import type { PlanningSnapshot, ScheduleEntry, Task } from "../api/planning";
import { copy } from "../copy";
import {
  planningCalendarDays,
  samePlanningDay,
  scheduleOverlapsPlanningDay,
  type PlanningViewRange,
} from "../planningRange";
import {
  isAllDaySchedule,
  layoutWeekDay,
  weekTimeWindow,
  type WeekTimedItem,
} from "../planning-week-layout";
import type { CalendarReport } from "./planning-calendar";

interface PlanningWeekCalendarProps {
  range: PlanningViewRange;
  snapshot: PlanningSnapshot | undefined;
  reports?: CalendarReport[];
  loading: boolean;
  onSelectDate(date: Date): void;
  onOpenSchedule?(entry: ScheduleEntry): void;
  onOpenTask?(task: Task): void;
  onOpenReport?(id: string, date: Date): void;
}
const HOUR_HEIGHT = 64;
const fullDate = new Intl.DateTimeFormat("ko-KR", {
  year: "numeric",
  month: "long",
  day: "numeric",
  weekday: "long",
});
const weekday = new Intl.DateTimeFormat("ko-KR", { weekday: "short" });
const clock = new Intl.DateTimeFormat("ko-KR", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});

export function PlanningWeekCalendar({
  range,
  snapshot,
  reports,
  loading,
  onSelectDate,
  onOpenSchedule,
  onOpenTask,
  onOpenReport,
}: PlanningWeekCalendarProps) {
  const hintId = useId();
  const dateButtons = useRef<Array<HTMLButtonElement | null>>([]);
  const restoreFocus = useRef(false);
  const days = planningCalendarDays(range);
  const schedule = (snapshot?.schedule ?? []).filter(
    (entry) => entry.status !== "cancelled",
  );
  const timed: WeekTimedItem[] = reports
    ? reports.map((report) => ({
        ...report,
        kind: "report",
        startsAt: report.receivedAt,
      }))
    : schedule
        .filter((entry) => !isAllDaySchedule(entry))
        .map((entry) => ({ ...entry, kind: "schedule", entry }));
  const columns = days.map((day) => layoutWeekDay(timed, day));
  const { startHour, endHour } = weekTimeWindow(columns.flat());
  const height = (endHour - startHour) * HOUR_HEIGHT;
  const hours = Array.from(
    { length: endHour - startHour },
    (_, index) => startHour + index,
  );
  const selectedIndex = days.findIndex((day) =>
    samePlanningDay(day, range.anchor),
  );
  const today = new Date();
  useEffect(() => {
    if (!loading && restoreFocus.current) {
      dateButtons.current[selectedIndex]?.focus({ preventScroll: true });
      restoreFocus.current = false;
    }
  }, [loading, selectedIndex, range.anchor.getTime()]);
  function select(day: Date) {
    restoreFocus.current = true;
    onSelectDate(day);
  }
  function move(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const next =
      event.key === "ArrowLeft"
        ? index - 1
        : event.key === "ArrowRight"
          ? index + 1
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? 6
              : undefined;
    if (next === undefined) return;
    event.preventDefault();
    dateButtons.current[Math.min(6, Math.max(0, next))]?.focus();
  }
  return (
    <section
      className="planning-calendar planning-week"
      data-mode="week"
      aria-label={copy.schedule.weekCalendar}
      aria-busy={loading}
    >
      <p id={hintId} className="planning-week__hint">
        {copy.schedule.weekCalendarHint}
      </p>
      <div
        className="planning-week__scroll"
        role="region"
        aria-label={copy.schedule.weekTimeGrid}
        aria-describedby={hintId}
        tabIndex={0}
      >
        <div className="planning-week__canvas">
          <div className="planning-week__header">
            <span className="planning-week__axis-title">
              {copy.schedule.calendarTime}
            </span>
            {days.map((day, index) => {
              const tasks = (snapshot?.tasks ?? []).filter(
                (task) =>
                  task.status === "open" &&
                  task.dueAt &&
                  samePlanningDay(new Date(task.dueAt), day),
              );
              const daySchedule = schedule.filter((entry) =>
                scheduleOverlapsPlanningDay(entry, day),
              );
              const dayReports = (reports ?? []).filter((report) =>
                samePlanningDay(new Date(report.receivedAt), day),
              );
              return (
                <button
                  key={day.getTime()}
                  ref={(element) => {
                    dateButtons.current[index] = element;
                  }}
                  className="planning-week__date focus-visible-control"
                  type="button"
                  data-selected={index === selectedIndex}
                  data-today={samePlanningDay(day, today)}
                  aria-pressed={index === selectedIndex}
                  aria-current={
                    samePlanningDay(day, today) ? "date" : undefined
                  }
                  tabIndex={index === selectedIndex ? 0 : -1}
                  disabled={loading}
                  aria-label={
                    reports
                      ? copy.projects.inflowHomeCalendarDate(
                          fullDate.format(day),
                          dayReports.length,
                        )
                      : copy.schedule.calendarDateLabel(
                          fullDate.format(day),
                          tasks.length,
                          daySchedule.length,
                        )
                  }
                  onKeyDown={(event) => move(event, index)}
                  onClick={() => select(day)}
                >
                  <span>{weekday.format(day)}</span>
                  <span className="planning-calendar__number">
                    {day.getDate()}
                  </span>
                </button>
              );
            })}
          </div>
          {!reports && (
            <div className="planning-week__all-day">
              <span className="planning-week__axis-title">
                {copy.schedule.calendarAllDay}
              </span>
              {days.map((day) => (
                <div
                  className="planning-week__all-day-column"
                  key={day.getTime()}
                >
                  {(snapshot?.tasks ?? [])
                    .filter(
                      (task) =>
                        task.status === "open" &&
                        task.dueAt &&
                        samePlanningDay(new Date(task.dueAt), day),
                    )
                    .map((task) => (
                      <button
                        className="planning-week__chip focus-visible-control"
                        data-kind="task"
                        type="button"
                        key={task.id}
                        disabled={loading}
                        title={task.title}
                        aria-label={copy.home.editTask(task.title)}
                        onClick={() =>
                          onOpenTask ? onOpenTask(task) : select(day)
                        }
                      >
                        {task.title}
                      </button>
                    ))}
                  {schedule
                    .filter(
                      (entry) =>
                        isAllDaySchedule(entry) &&
                        scheduleOverlapsPlanningDay(entry, day),
                    )
                    .map((entry) => (
                      <button
                        className="planning-week__chip focus-visible-control"
                        data-kind="schedule"
                        data-source={entry.source}
                        type="button"
                        key={entry.id}
                        disabled={loading}
                        title={entry.title}
                        onClick={() =>
                          entry.editable && onOpenSchedule
                            ? onOpenSchedule(entry)
                            : select(day)
                        }
                      >
                        {entry.title}
                      </button>
                    ))}
                </div>
              ))}
            </div>
          )}
          <div
            className="planning-week__body"
            style={
              {
                height: height + 44,
                "--week-hour-height": `${HOUR_HEIGHT}px`,
              } as CSSProperties
            }
          >
            <div className="planning-week__axis" aria-hidden="true">
              {hours.map((hour) => (
                <span
                  key={hour}
                  style={{ top: (hour - startHour) * HOUR_HEIGHT }}
                >
                  {String(hour).padStart(2, "0")}:00
                </span>
              ))}
            </div>
            {days.map((day, index) => (
              <div
                className="planning-week__column"
                data-selected={index === selectedIndex}
                key={day.getTime()}
              >
                {columns[index].map((block) => {
                  const top =
                    ((block.startMinute - startHour * 60) / 60) * HOUR_HEIGHT;
                  const blockHeight = Math.max(
                    44,
                    ((block.endMinute - block.startMinute) / 60) * HOUR_HEIGHT,
                  );
                  const label =
                    block.kind === "report"
                      ? copy.schedule.calendarReceivedTime(
                          clock.format(new Date(block.startsAt)),
                        )
                      : `${clock.format(new Date(block.startsAt))}–${clock.format(new Date(block.endsAt!))}`;
                  return (
                    <button
                      className="planning-week__event focus-visible-control"
                      type="button"
                      key={block.id}
                      data-kind={block.kind}
                      data-source={block.entry?.source}
                      disabled={loading}
                      style={{
                        top,
                        height: blockHeight,
                        left: `calc(${(block.column / block.columns) * 100}% + 4px)`,
                        width: `calc(${100 / block.columns}% - 8px)`,
                      }}
                      title={`${block.title}\n${label}`}
                      aria-label={`${block.title}, ${fullDate.format(day)}, ${label}`}
                      onClick={() =>
                        block.kind === "report" && onOpenReport
                          ? onOpenReport(block.id, day)
                          : block.entry?.editable && onOpenSchedule
                            ? onOpenSchedule(block.entry)
                            : select(day)
                      }
                    >
                      <strong>{block.title}</strong>
                      <span>{label}</span>
                    </button>
                  );
                })}
              </div>
            ))}
            {!columns.some((column) => column.length > 0) && (
              <p className="planning-week__empty">
                {copy.schedule.weekTimeEmpty}
              </p>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
