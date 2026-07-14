import { type Task } from "./api/planning";

export type TaskDueState = "overdue" | "today" | "tomorrow" | "later" | "none";

export function taskDueState(
  task: Pick<Task, "dueAt">,
  now = new Date(),
): TaskDueState {
  if (!task.dueAt) return "none";
  const dueAt = new Date(task.dueAt);
  if (Number.isNaN(dueAt.getTime())) return "none";
  if (dueAt < now) return "overdue";

  const startOfTomorrow = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate() + 1,
  );
  const startOfDayAfterTomorrow = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate() + 2,
  );
  if (dueAt < startOfTomorrow) return "today";
  if (dueAt < startOfDayAfterTomorrow) return "tomorrow";
  return "later";
}

export function deadlineAttentionTasks(
  tasks: Task[],
  now = new Date(),
): Task[] {
  const rank: Record<TaskDueState, number> = {
    overdue: 0,
    today: 1,
    tomorrow: 2,
    later: 3,
    none: 4,
  };
  return tasks
    .filter((task) => {
      const state = taskDueState(task, now);
      return state === "overdue" || state === "today" || state === "tomorrow";
    })
    .sort((left, right) => {
      const stateDifference =
        rank[taskDueState(left, now)] - rank[taskDueState(right, now)];
      if (stateDifference !== 0) return stateDifference;
      return (
        new Date(left.dueAt ?? 0).getTime() -
        new Date(right.dueAt ?? 0).getTime()
      );
    });
}
