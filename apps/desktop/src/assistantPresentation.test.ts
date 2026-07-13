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
  it("서버가 확정한 일감 순서와 ID를 그대로 사용한다", () => {
    const result = presentationForMessage(
      assistantMessage({
        kind: "tasks",
        title: "오늘 할 일",
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
        ],
      }),
    );

    expect(result.kind).toBe("tasks");
    if (result.kind !== "tasks") throw new Error("expected task result");
    expect(result.items.map((item) => item.id)).toEqual(["task-2", "task-1"]);
    expect(result.highlightedTaskId).toBe("task-2");
  });

  it("프레젠테이션이 없는 이전 메시지는 텍스트 요약으로만 표시한다", () => {
    expect(presentationForMessage(assistantMessage(null))).toEqual({
      kind: "summary",
      title: "요청 결과",
      summary: "서버에서 관련 항목을 확인했어요.",
    });
  });

  it("종류와 맞지 않는 서버 항목은 렌더링하지 않는다", () => {
    const result = presentationForMessage(
      assistantMessage({
        kind: "schedule",
        title: "오늘 일정",
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

    expect(result.kind).toBe("schedule");
    if (result.kind !== "schedule") throw new Error("expected schedule result");
    expect(result.items).toEqual([]);
  });
});
