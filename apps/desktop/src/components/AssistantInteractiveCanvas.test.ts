import { describe, expect, it } from "vitest";

import { type AssistantPresentationItem } from "../api/agent";
import { groupTaskPresentationItems } from "../assistantTaskGrouping";
import { canOpenPresentationItem } from "./AssistantInteractiveCanvas";

function project(status: "active" | "removed"): AssistantPresentationItem {
  return {
    type: "project",
    id: "019bffff-ffff-7fff-8000-000000000001",
    workspaceId: "019bffff-ffff-7fff-8000-000000000002",
    title: "비스킷링크",
    status,
    objective: null,
    nextAction: null,
    riskLevel: 0,
    openTaskCount: 0,
  };
}

describe("assistant project result navigation", () => {
  it("does not offer navigation after the project has been removed", () => {
    expect(canOpenPresentationItem(project("active"))).toBe(true);
    expect(canOpenPresentationItem(project("removed"))).toBe(false);
  });
});

describe("assistant task result grouping", () => {
  const tasks: Extract<AssistantPresentationItem, { type: "task" }>[] = [
    {
      type: "task",
      id: "019bffff-ffff-7fff-8000-000000000011",
      projectId: "019bffff-ffff-7fff-8000-000000000021",
      projectTitle: "비스킷링크",
      assigneeName: "김경주",
      title: "결제 흐름 확인",
      status: "open",
      priority: 2,
      dueAt: "2026-07-24T14:59:59Z",
    },
    {
      type: "task",
      id: "019bffff-ffff-7fff-8000-000000000012",
      projectId: "019bffff-ffff-7fff-8000-000000000021",
      projectTitle: "비스킷링크",
      assigneeName: "주홍석",
      title: "권한 정책 확인",
      status: "open",
      priority: 1,
      dueAt: "2026-07-23T14:59:59Z",
    },
    {
      type: "task",
      id: "019bffff-ffff-7fff-8000-000000000013",
      projectId: null,
      projectTitle: null,
      assigneeName: null,
      title: "담당자 정하기",
      status: "open",
      priority: 3,
      dueAt: null,
    },
    {
      type: "task",
      id: "019bffff-ffff-7fff-8000-000000000014",
      projectId: "019bffff-ffff-7fff-8000-000000000021",
      projectTitle: "비스킷링크",
      assigneeName: "김경주",
      title: "정산 검증",
      status: "open",
      priority: 3,
      dueAt: "2026-07-24T14:59:59Z",
    },
  ];

  it("담당자마다 모든 할 일을 분리하고 미정 항목을 마지막에 둔다", () => {
    const groups = groupTaskPresentationItems(tasks, "assignee");

    expect(groups.map((group) => [group.title, group.items.length])).toEqual([
      ["김경주", 2],
      ["주홍석", 1],
      ["담당자 미정", 1],
    ]);
    expect(groups[0]?.items.map((item) => item.title)).toEqual([
      "정산 검증",
      "결제 흐름 확인",
    ]);
  });

  it("기한 일자마다 모든 할 일을 분리하고 기한 없는 항목을 마지막에 둔다", () => {
    const groups = groupTaskPresentationItems(
      tasks,
      "date",
      new Date("2026-07-23T09:00:00+09:00"),
    );

    expect(groups.map((group) => [group.title, group.items.length])).toEqual([
      ["오늘 · 7월 23일 (목)", 1],
      ["내일 · 7월 24일 (금)", 2],
      ["기한 없음", 1],
    ]);
  });
});
