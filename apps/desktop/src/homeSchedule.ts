import { type ScheduleEntry } from "./api/planning";

export function upcomingHomeSchedules(
  entries: ScheduleEntry[],
  now = new Date(),
): ScheduleEntry[] {
  const threshold = now.getTime();
  return entries
    .filter((entry) => {
      if (entry.status !== "confirmed") return false;
      const startsAt = new Date(entry.startsAt).getTime();
      const endsAt = new Date(entry.endsAt).getTime();
      return (
        Number.isFinite(startsAt) &&
        Number.isFinite(endsAt) &&
        endsAt > threshold
      );
    })
    .sort(
      (left, right) =>
        new Date(left.startsAt).getTime() - new Date(right.startsAt).getTime(),
    );
}

export function localDayKey(now = new Date()): string {
  return [now.getFullYear(), now.getMonth() + 1, now.getDate()].join("-");
}

export function millisecondsUntilNextLocalDay(now = new Date()): number {
  const nextDay = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate() + 1,
  );
  return Math.max(1, nextDay.getTime() - now.getTime());
}
