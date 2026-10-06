import { describe, expect, it } from "vitest";

import { type Task } from "../api/planning";
import {
  scheduleLinkageForTask,
  scheduleTaskOptionLabel,
  type ScheduleProjectReference,
} from "./scheduleLinkage";

const task = {
  id: "019f68cb-9400-7000-8000-000000000022",
  projectId: "019f68cb-9400-7000-8000-000000000021",
  title: "계약서 검토",
} as Task;
const project = {
  id: "019f68cb-9400-7000-8000-000000000021",
  title: "비스킷링크",
  workspaceId: "workspace-company",
  workspaceName: "회사",
} satisfies ScheduleProjectReference;
const labels = {
  noProject: "프로젝트 없음",
  unknownProject: "프로젝트를 확인할 수 없음",
  unassigned: "담당자 미정",
  noDueDate: "기한 없음",
};

describe("schedule task linkage", () => {
  it("keeps the task and its project together", () => {
    expect(scheduleLinkageForTask([task], task.id)).toEqual({
      projectId: task.projectId,
      taskId: task.id,
    });
  });

  it("clears both links when no task is selected", () => {
    expect(scheduleLinkageForTask([task], "")).toEqual({
      projectId: null,
      taskId: null,
    });
  });

  it("distinguishes otherwise identical tasks by project", () => {
    const otherProject = {
      ...project,
      id: "019f68cb-9400-7000-8000-000000000024",
      title: "Jimin OS",
      workspaceId: "workspace-personal",
      workspaceName: "개인",
    };
    const sharedFields = {
      ...task,
      assigneeName: "김경주",
      dueAt: "2026-08-01T00:00:00Z",
    };

    expect(scheduleTaskOptionLabel(sharedFields, [project], labels)).toBe(
      "계약서 검토 · 회사 / 비스킷링크 · 김경주 · 8. 1.",
    );
    expect(
      scheduleTaskOptionLabel(
        {
          ...sharedFields,
          id: "019f68cb-9400-7000-8000-000000000023",
          projectId: otherProject.id,
        },
        [project, otherProject],
        labels,
      ),
    ).toBe("계약서 검토 · 개인 / Jimin OS · 김경주 · 8. 1.");
  });

  it("distinguishes projects with the same title across workspaces", () => {
    const personalProject = {
      ...project,
      id: "019f68cb-9400-7000-8000-000000000025",
      workspaceId: "workspace-personal",
      workspaceName: "개인",
    };
    const sharedFields = {
      ...task,
      assigneeName: "김경주",
      dueAt: "2026-08-01T00:00:00Z",
    };

    expect(scheduleTaskOptionLabel(sharedFields, [project], labels)).not.toBe(
      scheduleTaskOptionLabel(
        { ...sharedFields, projectId: personalProject.id },
        [project, personalProject],
        labels,
      ),
    );
  });

  it("labels tasks without a known project clearly", () => {
    expect(
      scheduleTaskOptionLabel(
        { ...task, projectId: null, assigneeName: null, dueAt: null },
        [project],
        labels,
      ),
    ).toBe("계약서 검토 · 프로젝트 없음 · 담당자 미정 · 기한 없음");
    expect(
      scheduleTaskOptionLabel(
        {
          ...task,
          projectId: "019f68cb-9400-7000-8000-000000000099",
          assigneeName: null,
          dueAt: null,
        },
        [project],
        labels,
      ),
    ).toBe("계약서 검토 · 프로젝트를 확인할 수 없음 · 담당자 미정 · 기한 없음");
  });
});
