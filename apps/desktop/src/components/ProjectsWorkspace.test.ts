import { describe, expect, it } from "vitest";

import { type Task } from "../api/planning";
import { taskHierarchyRows } from "./ProjectsWorkspace";

function task(
  id: string,
  title: string,
  parentTaskId: string | null = null,
): Task {
  return {
    id,
    projectId: "project",
    parentTaskId,
    title,
    notes: null,
    assigneeName: null,
    status: "open",
    priority: 1,
    dueAt: null,
    completedAt: null,
    version: 1,
  };
}

describe("project task hierarchy", () => {
  it("places child work immediately below its parent", () => {
    const parent = task("parent", "A 작업");
    const other = task("other", "B 작업");
    const child = task("child", "A-1 상세 기능", parent.id);

    expect(taskHierarchyRows([parent, other, child])).toEqual([
      { task: parent, depth: 0, childCount: 1 },
      { task: child, depth: 1, childCount: 0 },
      { task: other, depth: 0, childCount: 0 },
    ]);
  });

  it("keeps a task visible when its parent is no longer in the current view", () => {
    const child = task("child", "남아 있는 하위 일", "completed-parent");

    expect(taskHierarchyRows([child])).toEqual([
      { task: child, depth: 0, childCount: 0 },
    ]);
  });
});
