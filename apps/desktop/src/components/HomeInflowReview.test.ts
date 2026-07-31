import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ProjectInflowItem } from "../api/googleChat";
import { copy } from "../copy";
import {
  HomeInflowReview,
  homeInflowPendingItems,
  resolveHomeInflowSelection,
  visibleHomeInflowItems,
} from "./HomeInflowReview";

function inflow(
  id: string,
  overrides: Partial<ProjectInflowItem> = {},
): ProjectInflowItem {
  return {
    id,
    conversationId: id,
    representativeItemId: id,
    projectId: "project",
    projectName: "프로젝트",
    sourceId: "source",
    sourceName: "Google Chat",
    contentText: `업무 요청 ${id}`,
    receivedAt: "2026-07-31T01:00:00Z",
    suggestedTaskTitle: `업무 ${id}`,
    suggestedTaskNotes: `업무 ${id}를 처리합니다.`,
    suggestedPriority: 1,
    suggestedAssigneeName: null,
    suggestedDueAt: null,
    analysisStatus: "ready",
    analysisClassification: "new_task",
    status: "pending",
    promotedTaskId: null,
    sourceRevision: 1,
    analyzedRevision: 1,
    version: 1,
    ...overrides,
  } as ProjectInflowItem;
}

describe("home inflow review", () => {
  it("offers the full queue instead of silently truncating after five items", () => {
    const items = Array.from({ length: 7 }, (_, index) =>
      inflow(`${index + 1}`),
    );
    const markup = renderReview(items);

    expect(markup).toContain(copy.projects.inflowHomeShowAll(7));
    expect(markup).toContain('aria-expanded="false"');
    expect(markup).toContain("업무 5");
    expect(markup).not.toContain("업무 7");
    expect(visibleHomeInflowItems(items, true)).toHaveLength(7);
  });

  it("keeps new replies for an existing task visible while removing processed items", () => {
    const pending = inflow("pending");
    const followUp = inflow("follow-up", {
      promotedTaskId: "existing-task",
    });
    const promoted = inflow("promoted", {
      status: "promoted",
      promotedTaskId: "task",
    });
    const dismissed = inflow("dismissed", { status: "dismissed" });

    expect(
      homeInflowPendingItems([pending, followUp, promoted, dismissed]),
    ).toEqual([pending, followUp]);
    expect(renderReview([followUp])).toContain(
      copy.projects.inflowFollowUpTitle,
    );
    expect(renderReview([followUp])).toContain(
      copy.projects.inflowFollowUpOpenTask,
    );
    expect(renderReview([promoted, dismissed])).toBe("");
  });

  it("falls back to a visible conversation when filtering or collapsing removes the selection", () => {
    const items = Array.from({ length: 7 }, (_, index) =>
      inflow(`${index + 1}`),
    );
    const collapsed = visibleHomeInflowItems(items, false);

    expect(resolveHomeInflowSelection(collapsed, "7")).toBe(collapsed[0]);
  });
});

function renderReview(items: ProjectInflowItem[]): string {
  const props: ComponentProps<typeof HomeInflowReview> = {
    items,
    saving: false,
    onPromote: async () => undefined,
    onDismiss: async () => undefined,
    onRetryAnalysis: async () => undefined,
    onRetryCompletion: async () => undefined,
    onOpenTask: async () => undefined,
  };
  return renderToStaticMarkup(createElement(HomeInflowReview, props));
}
