const HISTORY_MONTHS = 3;
const FUTURE_DAYS = 90;

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
