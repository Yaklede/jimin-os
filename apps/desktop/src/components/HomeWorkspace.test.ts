import { describe, expect, it } from "vitest";
import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";

import { type Task } from "../api/planning";
import { type WeeklyReport } from "../api/projects";
import { HomeChatEntry } from "./HomeChatEntry";
import { selectWeeklyPriorityTasks } from "./HomeWorkspace";

it("limits deadline badge styling to the status label, not the task copy", () => {
  const styles = readFileSync(
    new URL("../styles.css", import.meta.url),
    "utf8",
  );
  const component = readFileSync(
    new URL("./HomeWorkspace.tsx", import.meta.url),
    "utf8",
  );
  expect(styles).not.toMatch(/\.home-deadline-brief__task\s*>\s*span\b/);
  expect(styles).toMatch(
    /\.home-deadline-brief__task\s*>\s*\.home-deadline-brief__due-state/,
  );
  expect(component).toContain('className="home-deadline-brief__due-state"');
  expect(component).toContain('className="home-deadline-brief__copy"');
});

describe("home new request action", () => {
  const base: ComponentProps<typeof HomeChatEntry> = {
    ready: true,
    draft: { text: "" },
    setDraft() {},
    onOpenHistory() {},
    onSend: async () => true,
  };

  it("offers a new question and a navigation button, without rendering history on home", () => {
    const markup = renderToStaticMarkup(createElement(HomeChatEntry, base));
    expect(markup).toContain("새로 물어볼 내용을 입력하세요");
    expect(markup).toContain('aria-label="대화 목록"');
    expect(markup).not.toContain("aria-expanded");
    expect(markup).not.toContain("최근 대화");
    expect(markup).not.toContain("결과 보기");
    expect(markup).not.toContain("이어서 요청하기");
  });

  it("renders the home draft after navigating back", () => {
    const markup = renderToStaticMarkup(
      createElement(HomeChatEntry, {
        ...base,
        draft: { text: "이어서 쓸 초안" },
      }),
    );
    expect(markup).toContain('value="이어서 쓸 초안"');
    expect(markup.match(/<input[^>]+>/)?.[0]).not.toContain("disabled");
  });
});

function task(
  id: string,
  projectId: string | null,
  status: Task["status"] = "open",
): Task {
  return {
    id,
    projectId,
    title: id,
    notes: null,
    status,
    priority: 2,
    dueAt: "2026-07-25T09:00:00Z",
    completedAt: null,
    version: 1,
  };
}

function report(): WeeklyReport {
  return {
    workspaceId: "workspace-company",
    periodStart: "2026-07-20",
    periodEnd: "2026-07-26",
    createdTaskCount: 5,
    completedTaskCount: 2,
    backlogStartCount: 4,
    backlogEndCount: 7,
    backlogDelta: 3,
    overdueTaskCount: 2,
    staleTaskCount: 1,
    unassignedTaskCount: 0,
    projects: [
      {
        projectId: "project-attention",
        title: "확인이 필요한 프로젝트",
        managementMode: "operation",
        createdTaskCount: 4,
        completedTaskCount: 1,
        backlogStartCount: 3,
        backlogEndCount: 6,
        backlogDelta: 3,
        overdueTaskCount: 2,
        staleTaskCount: 1,
        unassignedTaskCount: 0,
        averageCycleTimeHours: 8,
        onTimeCompletionPercent: 50,
        health: "needs_attention",
      },
      {
        projectId: "project-on-track",
        title: "순조로운 프로젝트",
        managementMode: "completion",
        createdTaskCount: 1,
        completedTaskCount: 1,
        backlogStartCount: 1,
        backlogEndCount: 1,
        backlogDelta: 0,
        overdueTaskCount: 0,
        staleTaskCount: 0,
        unassignedTaskCount: 0,
        averageCycleTimeHours: 4,
        onTimeCompletionPercent: 100,
        health: "on_track",
      },
    ],
  };
}

describe("weekly priority task selection", () => {
  it("keeps only open tasks from projects that need attention", () => {
    const selected = selectWeeklyPriorityTasks(
      [report()],
      [
        task("attention-open", "project-attention"),
        task("attention-completed", "project-attention", "completed"),
        task("on-track-open", "project-on-track"),
        task("unlinked-open", null),
      ],
    );

    expect(selected.map((item) => item.id)).toEqual(["attention-open"]);
  });

  it("limits the mobile priority summary to three tasks", () => {
    const selected = selectWeeklyPriorityTasks(
      [report()],
      Array.from({ length: 5 }, (_, index) =>
        task(`attention-${index + 1}`, "project-attention"),
      ),
    );

    expect(selected).toHaveLength(3);
  });
});
