import { type Task } from "../api/planning";

export type ScheduleLinkageInput = {
  projectId: string | null;
  taskId: string | null;
};

export type ScheduleProjectReference = {
  id: string;
  title: string;
  workspaceId: string;
  workspaceName: string;
};

export function scheduleLinkageForTask(
  tasks: Task[],
  taskId: string,
): ScheduleLinkageInput {
  const task = tasks.find((item) => item.id === taskId);
  return {
    projectId: task?.projectId ?? null,
    taskId: task?.id ?? null,
  };
}

export function scheduleTaskOptionLabel(
  task: Task,
  projects: ScheduleProjectReference[],
  labels: {
    noProject: string;
    unknownProject: string;
    unassigned: string;
    noDueDate: string;
  },
): string {
  const project = task.projectId
    ? projects.find((item) => item.id === task.projectId)
    : undefined;
  const projectName = task.projectId
    ? project
      ? `${project.workspaceName} / ${project.title}`
      : labels.unknownProject
    : labels.noProject;
  const assignee = task.assigneeName?.trim() || labels.unassigned;
  const dueDate = task.dueAt
    ? new Intl.DateTimeFormat("ko-KR", {
        timeZone: "Asia/Seoul",
        month: "numeric",
        day: "numeric",
      }).format(new Date(task.dueAt))
    : labels.noDueDate;
  return `${task.title} · ${projectName} · ${assignee} · ${dueDate}`;
}
