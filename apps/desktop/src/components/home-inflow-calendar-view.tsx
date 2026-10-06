import { ChevronLeft, ChevronRight } from "lucide-react";
import type { ProjectInflowItem } from "../api/googleChat";
import { copy } from "../copy";
import {
  planningViewRange,
  shiftPlanningViewRange,
  type PlanningViewRange,
} from "../planningRange";
import { PlanningCalendar } from "./planning-calendar";

interface HomeInflowCalendarViewProps {
  range: PlanningViewRange;
  items: ProjectInflowItem[];
  onRangeChange(range: PlanningViewRange): void;
  onSelectDate(date: Date): void;
  onSelectItem(item: ProjectInflowItem): void;
}

export function HomeInflowCalendarView({
  range,
  items,
  onRangeChange,
  onSelectDate,
  onSelectItem,
}: HomeInflowCalendarViewProps) {
  const monthLabel = new Intl.DateTimeFormat("ko-KR", {
    year: "numeric",
    month: "long",
  }).format(range.anchor);
  const dayLabel = new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
  });
  const label =
    range.mode === "month"
      ? monthLabel
      : `${dayLabel.format(range.from)}–${dayLabel.format(new Date(range.to.getTime() - 1))}`;
  return (
    <div className="home-inflow-calendar">
      <div className="home-inflow-calendar__toolbar">
        <div
          className="planning-range-tabs"
          role="group"
          aria-label={copy.schedule.rangeMode}
        >
          {(["week", "month"] as const).map((mode) => (
            <button
              key={mode}
              type="button"
              className="focus-visible-control"
              aria-pressed={range.mode === mode}
              data-active={range.mode === mode}
              onClick={() =>
                onRangeChange(planningViewRange(mode, range.anchor))
              }
            >
              {mode === "week"
                ? copy.schedule.weekRange
                : copy.schedule.monthRange}
            </button>
          ))}
        </div>
        <div
          className="home-inflow-calendar__navigation"
          role="group"
          aria-label={copy.schedule.rangeControls}
        >
          <button
            type="button"
            className="icon-button focus-visible-control"
            aria-label={copy.schedule.previousRange}
            onClick={() => onRangeChange(shiftPlanningViewRange(range, -1))}
          >
            <ChevronLeft aria-hidden="true" />
          </button>
          <strong aria-live="polite">{label}</strong>
          <button
            type="button"
            className="icon-button focus-visible-control"
            aria-label={copy.schedule.nextRange}
            onClick={() => onRangeChange(shiftPlanningViewRange(range, 1))}
          >
            <ChevronRight aria-hidden="true" />
          </button>
        </div>
        <button
          type="button"
          className="secondary-button focus-visible-control"
          onClick={() => onRangeChange(planningViewRange(range.mode))}
        >
          {copy.schedule.todayLabel}
        </button>
      </div>
      <p className="home-inflow-calendar__hint">
        {copy.projects.inflowHomeCalendarHint}
      </p>
      <PlanningCalendar
        range={range}
        snapshot={undefined}
        loading={false}
        reports={items.map((item) => ({
          id: item.id,
          title: item.suggestedTaskTitle,
          receivedAt: item.receivedAt,
        }))}
        onSelectDate={onSelectDate}
        onOpenReport={(id) => {
          const item = items.find((item) => item.id === id);
          if (item) onSelectItem(item);
        }}
      />
      <p className="home-inflow-calendar__hint">
        {copy.projects.inflowHomeCalendarScope}
      </p>
    </div>
  );
}
