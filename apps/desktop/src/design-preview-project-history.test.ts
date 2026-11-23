import { describe, expect, it } from "vitest";
import type { Task } from "./api/planning";
import {
  appendPreviewProjectHistory,
  previewProjectCompletedPage,
} from "./design-preview-project-history";

function history() {
  const now = Date.parse("2026-11-23T15:00:00Z");
  const tasks: Task[] = [
    {
      id: "latest",
      projectId: "preview-project",
      title: "마지막으로 완료한 일",
      notes: null,
      status: "completed",
      priority: 1,
      dueAt: null,
      completedAt: new Date(now).toISOString(),
      version: 1,
    },
  ];
  appendPreviewProjectHistory(tasks, now);
  return (params = "") =>
    previewProjectCompletedPage(
      tasks,
      "preview-project",
      new URLSearchParams(params),
    );
}

describe("completed project history preview", () => {
  it("loads 30 tasks in descending order and bounds the last page", () => {
    const load = history();
    const first = load();
    const last = load("page=300");
    expect(first.totalCount).toBe(390);
    expect(first.items).toHaveLength(30);
    expect(first.items[0]?.id).toBe("latest");
    expect(last.page).toBe(13);
    expect(last.items).toHaveLength(30);
    expect(last.items.at(-1)?.id).toBe("preview-archived-task-389");
    expect(load("anchorTaskId=preview-archived-task-389").page).toBe(13);
  });

  it("applies title, exact ticket, assignee and Seoul dates to the full history", () => {
    const load = history();
    const query = new URLSearchParams({ query: "#90389", assignee: "송천안" });
    expect(load(query.toString())).toMatchObject({
      totalCount: 390,
      filteredCount: 1,
      items: [{ id: "preview-archived-task-389" }],
    });
    query.set("query", "오래된 거래 승인");
    expect(load(query.toString()).filteredCount).toBe(1);
    query.set("completedFrom", "2026-11-23");
    expect(load(query.toString()).filteredCount).toBe(0);
    expect(
      load("completedFrom=2026-11-24&completedTo=2026-11-24").items,
    ).toEqual([expect.objectContaining({ id: "latest" })]);
  });
});
