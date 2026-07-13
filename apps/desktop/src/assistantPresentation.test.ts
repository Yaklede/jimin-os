import { describe, expect, it } from "vitest";

import { type ConversationMessage } from "./api/agent";
import { presentationForMessage } from "./assistantPresentation";

function assistantMessage(
  presentation: ConversationMessage["presentation"],
): ConversationMessage {
  return {
    id: "message-1",
    role: "assistant",
    content: "서버에서 관련 항목을 확인했어요.",
    presentation,
    status: "completed",
    createdAt: "2026-07-13T00:00:00Z",
    completedAt: "2026-07-13T00:00:01Z",
    version: 1,
  };
}

describe("presentationForMessage", () => {
  it("AI가 선택한 섹션, 순서, 초점 대상을 그대로 구성한다", () => {
    const result = presentationForMessage(
      assistantMessage({
        kind: "composite",
        title: "오늘의 실행 계획",
        layout: "focus",
        focusItemId: "task-1",
        sections: [
          {
            kind: "tasks",
            title: "먼저 할 일",
            view: "checklist",
            itemIds: ["task-2", "task-1"],
          },
          {
            kind: "schedule",
            title: "예정된 일정",
            view: "timeline",
            itemIds: ["schedule-1"],
          },
        ],
        items: [
          {
            type: "task",
            id: "task-2",
            projectId: null,
            projectTitle: null,
            title: "장보기",
            priority: 1,
            dueAt: null,
          },
          {
            type: "task",
            id: "task-1",
            projectId: "project-1",
            projectTitle: "개인 운영체제",
            title: "회의록 정리",
            priority: 2,
            dueAt: null,
          },
          {
            type: "schedule",
            id: "schedule-1",
            title: "주간 회의",
            startsAt: "2026-07-13T04:00:00Z",
            endsAt: "2026-07-13T05:00:00Z",
            timeZone: "Asia/Seoul",
          },
        ],
      }),
    );

    expect(result.layout).toBe("focus");
    expect(result.sections.map((section) => section.kind)).toEqual([
      "tasks",
      "schedule",
    ]);
    expect(result.sections[0]?.items.map((item) => item.id)).toEqual([
      "task-2",
      "task-1",
    ]);
    expect(result.focusItemId).toBe("task-1");
  });

  it("프레젠테이션이 없는 이전 메시지는 텍스트 요약으로만 표시한다", () => {
    expect(presentationForMessage(assistantMessage(null))).toEqual({
      title: "요청 결과",
      summary: "서버에서 관련 항목을 확인했어요.",
      layout: "stack",
      sections: [],
      focusItemId: undefined,
    });
  });

  it("이전 단일 목록 형식도 탐색 가능한 섹션으로 바꾼다", () => {
    const result = presentationForMessage(
      assistantMessage({
        kind: "tasks",
        title: "오늘 할 일",
        layout: "stack",
        focusItemId: null,
        sections: [],
        items: [
          {
            type: "task",
            id: "task-1",
            projectId: null,
            projectTitle: null,
            title: "일감",
            priority: 1,
            dueAt: null,
          },
        ],
      }),
    );

    expect(result.sections[0]?.kind).toBe("tasks");
    expect(result.sections[0]?.items[0]?.id).toBe("task-1");
    expect(result.focusItemId).toBe("task-1");
  });

  it("섹션 종류와 맞지 않는 항목은 렌더링하지 않는다", () => {
    const result = presentationForMessage(
      assistantMessage({
        kind: "schedule",
        title: "오늘 일정",
        layout: "split",
        focusItemId: "task-1",
        sections: [
          {
            kind: "schedule",
            title: "일정",
            view: "timeline",
            itemIds: ["task-1"],
          },
        ],
        items: [
          {
            type: "task",
            id: "task-1",
            projectId: null,
            projectTitle: null,
            title: "일감",
            priority: 1,
            dueAt: null,
          },
        ],
      }),
    );

    expect(result.sections).toEqual([]);
    expect(result.layout).toBe("stack");
    expect(result.focusItemId).toBeUndefined();
  });
});
