import type { ProjectInflowItem } from "./api/googleChat";
import { projectCopy } from "./copy/projects";
import { samePlanningDay } from "./planningRange";

const relativeTime = new Intl.RelativeTimeFormat("ko-KR", { numeric: "always" });

export function formatHomeInflowAge(receivedAtMs: number, nowMs: number) {
  const elapsedMs = nowMs - receivedAtMs;
  if (!Number.isFinite(elapsedMs) || elapsedMs < 0) return;

  const minutes = Math.floor(elapsedMs / 60_000);
  if (minutes < 1) return projectCopy.inflowHomeJustNow;
  if (minutes < 60) return relativeTime.format(-minutes, "minute");

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return relativeTime.format(-hours, "hour");
  return relativeTime.format(-Math.floor(hours / 24), "day");
}

// Unknown received dates remain available in the full list, never on a guessed day.
export function homeInflowOnDate(
  items: ProjectInflowItem[],
  date: Date,
): ProjectInflowItem[] {
  return items.filter((item) =>
    samePlanningDay(new Date(item.receivedAt), date),
  );
}

export function homeInflowByReceivedDate(
  items: ProjectInflowItem[],
): ProjectInflowItem[] {
  const timestamp = (item: ProjectInflowItem) => {
    const value = new Date(item.receivedAt).getTime();
    return Number.isFinite(value) ? value : -Infinity;
  };
  return [...items].sort((left, right) => timestamp(right) - timestamp(left));
}
