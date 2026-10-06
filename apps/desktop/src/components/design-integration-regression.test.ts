import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Task } from "../api/planning";
import { planningViewRange } from "../planningRange";
import { AssistantInteractiveCanvas } from "./AssistantInteractiveCanvas";
import { PlanningWorkspace } from "./PlanningWorkspace";

const task: Task = {
  id: "integration-task",
  projectId: null,
  title: "기존 기능 유지 확인",
  notes: "문서 링크와 담당자 정보를 유지합니다.",
  assigneeName: "김경주",
  status: "open",
  priority: 2,
  dueAt: null,
  completedAt: null,
  version: 1,
};

describe("design integration production regression", () => {
  it("keeps a task selection control and explicit detail actions in assistant results", () => {
    const markup = renderToStaticMarkup(
      createElement(AssistantInteractiveCanvas, {
        presentation: {
          title: "할 일",
          summary: "기존 기능을 유지합니다.",
          layout: "split",
          focusItemId: task.id,
          sections: [
            {
              kind: "tasks",
              title: "할 일",
              view: "checklist",
              items: [{ ...task, type: "task", projectTitle: null }],
            },
          ],
        },
        onContinue() {},
        onLoadTask: async () => task,
        onCompleteTask: async () => task,
        onRestoreTask: async () => task,
        onEditTask() {},
        onEditSchedule() {},
        onOpenTask() {},
        onOpenProject() {},
        onOpenSchedule() {},
      }),
    );
    expect(markup).toContain('role="checkbox"');
    expect(markup).toContain('aria-label="기존 기능 유지 확인 선택"');
    // Notes load asynchronously through onLoadTask; SSR checks the preserved action surface.
    expect(markup).toContain('aria-label="선택한 내용"');
    expect(markup).toContain("담당자: 김경주");
    expect(markup).toContain(">수정</button>");
    expect(markup).toContain(">완료</button>");
    expect(markup).not.toContain("완료 처리하기");
  });

  it("keeps both past schedules and completed tasks collapsed by default", () => {
    const markup = renderToStaticMarkup(
      createElement(PlanningWorkspace, {
        snapshot: {
          tasks: [task],
          schedule: [],
          completedTasks: [{ ...task, status: "completed" }],
        },
        projects: [],
        range: planningViewRange("day"),
        loading: false,
        error: undefined,
        onCompleteTask: async () => {},
        onRestoreTask: async () => {},
        onCreateTask: async () => {},
        onCreateSchedule: async () => {},
        onEditTask() {},
        onEditSchedule() {},
        onRangeChange: async () => {},
      }),
    );
    expect(markup.match(/<details class="planning-archive">/g)).toHaveLength(2);
    expect(markup).not.toMatch(/<details[^>]*open/);
    expect(markup).toContain("다시 진행하기");
    expect(markup).toContain('role="checkbox"');
  });
});
