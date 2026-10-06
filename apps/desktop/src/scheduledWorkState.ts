import type { ScheduledWorkRunPage } from "./api/scheduledWork";

export function mergeScheduledRuns(
  current: ScheduledWorkRunPage,
  latest: ScheduledWorkRunPage,
): ScheduledWorkRunPage {
  if (current.items.length <= 50) return latest;
  const latestIds = new Set(latest.items.map((run) => run.id));
  return {
    items: [
      ...latest.items,
      ...current.items.filter((run) => !latestIds.has(run.id)),
    ],
    nextCursor: current.nextCursor,
  };
}
export function latestScheduledFailure(
  page: ScheduledWorkRunPage,
  ids: string[],
) {
  const seen = new Set<string>();
  return page.items.find((run) => {
    if (!ids.includes(run.scheduledWorkId) || seen.has(run.scheduledWorkId))
      return false;
    seen.add(run.scheduledWorkId);
    return run.status === "failed";
  });
}
export function parseAssigneeFilter(value: string): string[] {
  return [
    ...new Set(
      value
        .split(",")
        .map((name) => name.trim())
        .filter(Boolean),
    ),
  ];
}
