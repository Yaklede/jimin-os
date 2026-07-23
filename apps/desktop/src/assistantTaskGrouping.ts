import { type AssistantPresentationSection } from "./assistantPresentation";
import { copy } from "./copy";

type TaskPresentationItem = Extract<
  AssistantPresentationSection["items"][number],
  { type: "task" }
>;

export type TaskGroupView = "assignee" | "date";

export type TaskPresentationGroup = {
  id: string;
  title: string;
  items: TaskPresentationItem[];
};

export function initialTaskGroupView(
  section: AssistantPresentationSection | undefined,
): TaskGroupView {
  if (section?.kind !== "tasks") return "assignee";
  return /일자|날짜|기한/.test(section.title) ? "date" : "assignee";
}

export function groupTaskPresentationItems(
  items: TaskPresentationItem[],
  view: TaskGroupView,
  now = new Date(),
): TaskPresentationGroup[] {
  const groups = new Map<string, TaskPresentationGroup>();
  for (const item of [...items].sort(compareTaskPresentationItems)) {
    const descriptor =
      view === "assignee"
        ? assigneeGroupDescriptor(item)
        : dateGroupDescriptor(item, now);
    const existing = groups.get(descriptor.id);
    if (existing) existing.items.push(item);
    else groups.set(descriptor.id, { ...descriptor, items: [item] });
  }
  const result = [...groups.values()];
  return result.sort((left, right) => {
    if (left.id.endsWith(":none")) return 1;
    if (right.id.endsWith(":none")) return -1;
    return left.id.localeCompare(right.id, "ko");
  });
}

export function taskGroupItemSummary(
  item: TaskPresentationItem,
  view: TaskGroupView,
) {
  const project = item.projectTitle || copy.home.unassignedTask;
  if (view === "date") {
    return `${project} · ${copy.projects.taskAssignee(item.assigneeName ?? undefined)}`;
  }
  return item.dueAt ? `${project} · ${formatTaskDate(item.dueAt)}` : project;
}

function assigneeGroupDescriptor(item: TaskPresentationItem) {
  const assignee = item.assigneeName?.trim();
  return assignee
    ? { id: `assignee:${assignee}`, title: assignee }
    : { id: "assignee:none", title: copy.home.unassignedTaskGroup };
}

function dateGroupDescriptor(item: TaskPresentationItem, now: Date) {
  if (!item.dueAt) {
    return { id: "date:none", title: copy.home.noDueDateTaskGroup };
  }
  const due = new Date(item.dueAt);
  if (Number.isNaN(due.getTime())) {
    return { id: "date:none", title: copy.home.noDueDateTaskGroup };
  }
  const key = dateKey(due);
  const todayKey = dateKey(now);
  const tomorrow = new Date(now);
  tomorrow.setDate(tomorrow.getDate() + 1);
  const relative =
    key === todayKey
      ? copy.home.todayTaskGroup
      : key === dateKey(tomorrow)
        ? copy.home.tomorrowTaskGroup
        : undefined;
  const date = formatTaskDate(item.dueAt);
  return {
    id: `date:${key}`,
    title: relative ? `${relative} · ${date}` : date,
  };
}

function dateKey(value: Date): string {
  const parts = new Intl.DateTimeFormat("en-CA", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    timeZone: "Asia/Seoul",
  }).formatToParts(value);
  const part = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((item) => item.type === type)?.value ?? "";
  return `${part("year")}-${part("month")}-${part("day")}`;
}

function compareTaskPresentationItems(
  left: TaskPresentationItem,
  right: TaskPresentationItem,
) {
  return (
    right.priority - left.priority ||
    sortableDate(left.dueAt) - sortableDate(right.dueAt) ||
    left.title.localeCompare(right.title, "ko")
  );
}

function sortableDate(value: string | null): number {
  if (!value) return Number.MAX_SAFE_INTEGER;
  const date = new Date(value).getTime();
  return Number.isNaN(date) ? Number.MAX_SAFE_INTEGER : date;
}

function formatTaskDate(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    weekday: "short",
    timeZone: "Asia/Seoul",
  }).format(new Date(value));
}
