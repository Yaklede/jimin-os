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
  it("separates new requests, existing task updates and reviewed conversations", () => {
    const markup = renderReview([
      inflow("new"),
      inflow("update", { promotedTaskId: "existing-task" }),
      inflow("read", { reviewed: true }),
    ]);
    expect(markup).toContain(copy.projects.inflowNewTitle);
    expect(markup).toContain(copy.projects.inflowExistingTitle);
    expect(markup).toContain(copy.projects.inflowReviewedTitle);
    expect(markup).toContain('<details class="project-inflow__history">');
    expect(markup).not.toContain(
      '<details open="" class="project-inflow__history">',
    );
    expect(markup).toContain('id="home-inflow-queue-title-new"');
    expect(markup).toContain('id="home-inflow-queue-title-existing"');
    expect(markup).toContain(copy.projects.inflowMarkSeen);
    expect(markup.match(/role="tablist"/g)).toHaveLength(1);
    expect(markup.match(/role="tab"/g)).toHaveLength(2);
    expect(markup.match(/role="tabpanel"/g)).toHaveLength(2);
    expect(markup).toContain('aria-controls="home-inflow-panel-new"');
    expect(markup).toContain('aria-controls="home-inflow-panel-existing"');
    expect(markup).toMatch(/id="home-inflow-tab-new"[^>]*aria-selected="true"/);
    expect(markup).toMatch(/id="home-inflow-panel-existing"[^>]*hidden=""/);
  });

  it("opens existing updates when there are no new requests", () => {
    const markup = renderReview([inflow("update", { promotedTaskId: "task" })]);
    expect(markup).toMatch(
      /id="home-inflow-tab-existing"[^>]*aria-selected="true"/,
    );
    expect(markup).toMatch(/id="home-inflow-panel-new"[^>]*hidden=""/);
    expect(markup).toContain(copy.projects.inflowNewEmpty);
  });
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
