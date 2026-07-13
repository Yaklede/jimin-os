import { describe, expect, it } from "vitest";

import { deriveAssistantPresentation } from "./assistantPresentation";
import { type HomeSnapshot } from "./api/home";
import { type Project } from "./api/projects";

const snapshot: HomeSnapshot = {
  schedule: [
    {
      id: "schedule-1",
      title: "제품 회의",
      notes: null,
      startsAt: "2026-07-13T01:00:00.000Z",
      endsAt: "2026-07-13T02:00:00.000Z",
      timeZone: "Asia/Seoul",
      status: "confirmed",
      version: 1,
    },
  ],
  tasks: [
    {
      id: "task-1",
      projectId: "project-1",
      title: "비스켓링크 회의록 정리",
      notes: null,
      status: "open",
      priority: 2,
      dueAt: null,
      completedAt: null,
      version: 1,
    },
    {
      id: "task-2",
      projectId: null,
      title: "장보기",
      notes: null,
      status: "open",
      priority: 1,
      dueAt: null,
      completedAt: null,
      version: 1,
    },
  ],
};

const projects: Project[] = [
  {
    id: "project-1",
    workspaceId: "workspace-1",
    title: "비스켓링크",
    objective: "회의 내용을 제품 작업으로 연결한다.",
    status: "active",
    riskLevel: 1,
    nextAction: "회의록 정리",
    dueAt: null,
    openTaskCount: 1,
    version: 1,
  },
];

describe("deriveAssistantPresentation", () => {
  it("관련 일감을 점수순으로 좁히고 첫 항목을 강조한다", () => {
    const result = deriveAssistantPresentation(
      "비스켓링크 일감 찾아줘",
      "관련 일감을 찾았어요.",
      snapshot,
      projects,
    );

    expect(result.kind).toBe("tasks");
    if (result.kind !== "tasks") throw new Error("expected task result");
    expect(result.items.map((item) => item.id)).toEqual(["task-1"]);
    expect(result.highlightedTaskId).toBe("task-1");
  });

  it("구체 검색어가 없으면 열린 일감 전체를 보여준다", () => {
    const result = deriveAssistantPresentation(
      "내 할 일 보여줘",
      "열린 할 일을 정리했어요.",
      snapshot,
      projects,
    );

    expect(result.kind).toBe("tasks");
    if (result.kind !== "tasks") throw new Error("expected task result");
    expect(result.items).toHaveLength(2);
  });

  it("일정 요청은 오늘 일정 표면으로 만든다", () => {
    const result = deriveAssistantPresentation(
      "오늘 일정 보여줘",
      "오늘 회의가 하나 있어요.",
      snapshot,
      projects,
    );

    expect(result.kind).toBe("schedule");
    if (result.kind !== "schedule") throw new Error("expected schedule result");
    expect(result.items[0]?.title).toBe("제품 회의");
  });

  it("일반 질문은 요약 결과로 유지한다", () => {
    const result = deriveAssistantPresentation(
      "오늘 무엇부터 하면 좋을까",
      "회의 준비부터 시작하는 게 좋아요.",
      snapshot,
      projects,
    );

    expect(result).toEqual({
      kind: "summary",
      title: "요청 결과",
      summary: "회의 준비부터 시작하는 게 좋아요.",
    });
  });
});
