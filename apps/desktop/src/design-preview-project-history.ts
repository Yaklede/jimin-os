import type { Task } from "./api/planning";
import {
  PROJECT_COMPLETED_PAGE_SIZE,
  type ProjectCompletedPage,
} from "./api/projects";

export function appendPreviewProjectHistory(tasks: Task[], now: number) {
  const topics = [
    "정산 내역 확인",
    "결제 오류 수정",
    "거래내역 검토",
    "로그인 문구 정리",
    "배정 알림 확인",
  ];
  for (let index = 1; index < 390; index += 1) {
    tasks.push({
      id: `preview-archived-task-${index}`,
      projectId: "preview-project",
      title:
        index === 389
          ? "예시 · 오래된 거래 승인 점검"
          : `예시 · ${topics[index % topics.length]} ${index}`,
      notes: `예시 자료: https://itsm.example/issues/${90000 + index}`,
      status: "completed",
      assigneeName: ["김경주", "송천안", "김경주, 송천안"][index % 3],
      priority: 1 + (index % 3),
      dueAt: null,
      completedAt: new Date(now - index * 86_400_000).toISOString(),
      version: 1,
    });
  }
}

const members = (task: Task) =>
  (task.assigneeName ?? "")
    .split(/[,，]/u)
    .map((name) => name.trim())
    .filter(Boolean);

function seoulDate(iso: string) {
  return new Date(Date.parse(iso) + 9 * 3_600_000).toISOString().slice(0, 10);
}

export function previewProjectCompletedPage(
  tasks: readonly Task[],
  projectId: string,
  params: URLSearchParams,
): ProjectCompletedPage {
  const all = tasks.filter(
    (task) => task.projectId === projectId && task.status === "completed",
  );
  const query = params.get("query")?.trim().toLowerCase() ?? "";
  const ticket =
    /^(?:itsm\s+)?#?(\d+)$/u.exec(query)?.[1] ??
    /^https?:\/\/[^/]+\/.*?issues\/(\d+)(?:[/?#]|$)/u.exec(query)?.[1];
  const assignee = params.get("assignee");
  const from = params.get("completedFrom");
  const to = params.get("completedTo");
  const anchor = params.get("anchorTaskId");
  const matches = all
    .filter((task) => {
      const textMatches = ticket
        ? new RegExp(`(?:/issues/|#)${Number(ticket)}(?:[^0-9]|$)`, "u").test(
            `${task.title} ${task.notes ?? ""}`,
          )
        : task.title.toLowerCase().includes(query);
      if (!textMatches || (assignee && !members(task).includes(assignee)))
        return false;
      if (!from && !to) return true;
      if (!task.completedAt) return false;
      const day = seoulDate(task.completedAt);
      return (!from || day >= from) && (!to || day <= to);
    })
    .sort(
      (left, right) =>
        (right.completedAt ?? "").localeCompare(left.completedAt ?? "") ||
        right.id.localeCompare(left.id),
    );
  const last = Math.max(
    1,
    Math.ceil(matches.length / PROJECT_COMPLETED_PAGE_SIZE),
  );
  const anchorIndex = anchor
    ? matches.findIndex((task) => task.id === anchor)
    : -1;
  const page =
    anchorIndex >= 0
      ? Math.floor(anchorIndex / PROJECT_COMPLETED_PAGE_SIZE) + 1
      : Math.min(last, Math.max(1, Number(params.get("page") ?? "1") || 1));
  const offset = (page - 1) * PROJECT_COMPLETED_PAGE_SIZE;
  return {
    items: matches.slice(offset, offset + PROJECT_COMPLETED_PAGE_SIZE),
    page,
    pageSize: PROJECT_COMPLETED_PAGE_SIZE,
    totalCount: all.length,
    filteredCount: matches.length,
    assignees: [...new Set(all.flatMap(members))].sort((left, right) =>
      left.localeCompare(right, "ko"),
    ),
  };
}
