const HISTORY_MONTHS = 3;
const FUTURE_DAYS = 90;

export type PlanningRangeMode = "day" | "week" | "month";

export interface PlanningViewRange {
  mode: PlanningRangeMode;
  anchor: Date;
  from: Date;
  to: Date;
}

export function samePlanningViewRange(
  left: PlanningViewRange,
  right: PlanningViewRange,
): boolean {
  return (
    left.mode === right.mode &&
    left.anchor.getTime() === right.anchor.getTime() &&
    left.from.getTime() === right.from.getTime() &&
    left.to.getTime() === right.to.getTime()
  );
}

export function planningViewRange(
  mode: PlanningRangeMode,
  anchor = new Date(),
): PlanningViewRange {
  const normalizedAnchor = startOfLocalDay(anchor);
  if (mode === "day") {
    return {
      mode,
      anchor: normalizedAnchor,
      from: normalizedAnchor,
      to: new Date(
        normalizedAnchor.getFullYear(),
        normalizedAnchor.getMonth(),
        normalizedAnchor.getDate() + 1,
      ),
    };
  }
  if (mode === "week") {
    const mondayOffset = (normalizedAnchor.getDay() + 6) % 7;
    const from = new Date(
      normalizedAnchor.getFullYear(),
      normalizedAnchor.getMonth(),
      normalizedAnchor.getDate() - mondayOffset,
    );
    return {
      mode,
      anchor: normalizedAnchor,
      from,
      to: new Date(from.getFullYear(), from.getMonth(), from.getDate() + 7),
    };
  }
  return {
    mode,
    anchor: normalizedAnchor,
    from: new Date(
      normalizedAnchor.getFullYear(),
      normalizedAnchor.getMonth(),
      1,
    ),
    to: new Date(
      normalizedAnchor.getFullYear(),
      normalizedAnchor.getMonth() + 1,
      1,
    ),
  };
}

export function shiftPlanningViewRange(
  range: PlanningViewRange,
  direction: -1 | 1,
): PlanningViewRange {
  const { anchor, mode } = range;
  const nextAnchor =
    mode === "month"
      ? new Date(anchor.getFullYear(), anchor.getMonth() + direction, 1)
      : new Date(
          anchor.getFullYear(),
          anchor.getMonth(),
          anchor.getDate() + direction * (mode === "week" ? 7 : 1),
        );
  return planningViewRange(mode, nextAnchor);
}

export function currentPlanningRange(
  targetStartsAt?: string,
  now = new Date(),
): [Date, Date] {
  const defaultFrom = new Date(
    now.getFullYear(),
    now.getMonth() - HISTORY_MONTHS,
    now.getDate(),
  );
  const defaultTo = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate() + FUTURE_DAYS,
  );
  if (!targetStartsAt) return [defaultFrom, defaultTo];

  const target = new Date(targetStartsAt);
  if (Number.isNaN(target.getTime())) return [defaultFrom, defaultTo];
  if (target < defaultFrom) {
    return [startOfLocalDay(target), defaultTo];
  }
  if (target >= defaultTo) {
    return [defaultFrom, endOfLocalDay(target)];
  }
  return [defaultFrom, defaultTo];
}

function startOfLocalDay(value: Date): Date {
  return new Date(value.getFullYear(), value.getMonth(), value.getDate());
}

function endOfLocalDay(value: Date): Date {
  return new Date(value.getFullYear(), value.getMonth(), value.getDate() + 1);
}

// Calendar cells follow the same Monday-based local-day boundaries as ranges.
export function planningCalendarDays(range: PlanningViewRange): Date[] {
  if (range.mode === "day") return [range.anchor];
  if (range.mode === "week") {
    return Array.from(
      { length: 7 },
      (_, offset) =>
        new Date(
          range.from.getFullYear(),
          range.from.getMonth(),
          range.from.getDate() + offset,
        ),
    );
  }
  const first = new Date(
    range.anchor.getFullYear(),
    range.anchor.getMonth(),
    1,
  );
  const offset = (first.getDay() + 6) % 7;
  const dayCount = new Date(
    first.getFullYear(),
    first.getMonth() + 1,
    0,
  ).getDate();
  const cellCount = Math.ceil((offset + dayCount) / 7) * 7;
  return Array.from(
    { length: cellCount },
    (_, index) =>
      new Date(first.getFullYear(), first.getMonth(), 1 - offset + index),
  );
}

export function samePlanningDay(left: Date, right: Date): boolean {
  return (
    left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate()
  );
}

export function scheduleOverlapsPlanningDay(
  entry: { startsAt: string; endsAt: string },
  day: Date,
): boolean {
  const start = startOfLocalDay(day);
  const end = endOfLocalDay(day);
  return new Date(entry.startsAt) < end && new Date(entry.endsAt) > start;
}
