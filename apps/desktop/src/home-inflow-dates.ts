import type { ProjectInflowItem } from "./api/googleChat";
import { samePlanningDay } from "./planningRange";

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
