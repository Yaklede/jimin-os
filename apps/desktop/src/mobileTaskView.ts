import type { Task } from "./api/planning";

export type MobileTaskFilter = "today" | "overdue" | "all";
export type MobileTaskGrouping = "assignee" | "date";

export function seoulTaskDay(value: string | Date): string {
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Seoul",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date(value));
}

export function filterMobileTasks(
  tasks: Task[],
  filter: MobileTaskFilter,
  now: Date,
): Task[] {
  const today = seoulTaskDay(now);
  return tasks
    .filter(
      (task) =>
        task.status === "open" &&
        (filter === "all" ||
          (filter === "today" &&
            Boolean(task.dueAt) &&
            seoulTaskDay(task.dueAt!) === today) ||
          (filter === "overdue" &&
            Boolean(task.dueAt) &&
            new Date(task.dueAt!).getTime() < now.getTime())),
    )
    .sort(
      (a, b) =>
        (a.dueAt ? new Date(a.dueAt).getTime() : Infinity) -
          (b.dueAt ? new Date(b.dueAt).getTime() : Infinity) ||
        b.priority - a.priority ||
        a.title.localeCompare(b.title, "ko"),
    );
}

export function groupMobileTasks(tasks: Task[], grouping: MobileTaskGrouping) {
  const groups = new Map<string, Task[]>();
  for (const task of tasks) {
    const label =
      grouping === "assignee"
        ? task.assigneeName?.trim() || "담당자 미정"
        : task.dueAt
          ? seoulTaskDay(task.dueAt)
          : "기한 없음";
    groups.set(label, [...(groups.get(label) ?? []), task]);
  }
  return [...groups].sort(([a], [b]) =>
    a === "담당자 미정" || a === "기한 없음"
      ? 1
      : b === "담당자 미정" || b === "기한 없음"
        ? -1
        : a.localeCompare(b, "ko"),
  );
}

export function mergeMobileTasks(
  all: Task[],
  recent: Task[],
  authoritative = false,
) {
  // Home and all-task snapshots can arrive in either order. Use the newest version.
  const tasks = new Map(all.map((task) => [task.id, task]));
  for (const task of recent) {
    // A successful all-open response excludes completed/deleted tasks. A stale
    // home snapshot must not add those IDs back; newly created work is reloaded.
    if (authoritative && !tasks.has(task.id)) continue;
    if ((tasks.get(task.id)?.version ?? -1) <= task.version)
      tasks.set(task.id, task);
  }
  return [...tasks.values()];
}
