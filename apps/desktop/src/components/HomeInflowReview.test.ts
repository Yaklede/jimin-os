import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import type { ProjectInflowItem } from "../api/googleChat";
import { copy } from "../copy";
import {
  inflowReviewSnapshotKey,
  markInflowSeenBatch,
  reconcileInflowSeenBatch,
} from "../home-inflow-batch";
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
  it("offers batch review only for existing updates and uncertain conversations", () => {
    const markup = renderReview([
      inflow("new"),
      inflow("update", { promotedTaskId: "task" }),
      inflow("question", { analysisClassification: "question" }),
    ]);
    const panels = markup.split('role="tabpanel"').slice(1);
    expect(panels[0]).not.toContain(copy.projects.inflowBulkSelect);
    expect(panels[1]).toContain(copy.projects.inflowBulkSelect);
    expect(panels[2]).toContain(copy.projects.inflowBulkSelect);
  });

  it("reviews each distinct selected snapshot, retaining failures and newer revisions", async () => {
    const selected = Array.from({ length: 7 }, (_, index) =>
      inflow(`item-${index}`),
    );
    const requests: string[] = [];
    let inFlight = 0;
    let peak = 0;
    const result = await markInflowSeenBatch(
      [...selected, selected[0]],
      async (item) => {
        requests.push(item.id);
        peak = Math.max(peak, ++inFlight);
        await Promise.resolve();
        inFlight--;
        if (item === selected[1]) throw new Error("conflict");
        return { ...item, reviewed: true, version: item.version + 1 };
      },
    );
    expect(requests).toEqual(selected.map((item) => item.id));
    expect(peak).toBe(3);
    expect(result.failed).toEqual([selected[1]]);
    expect(result.updatedVersions.size).toBe(6);
    const newer = { ...selected[0], sourceRevision: 2 };
    const unselected = inflow("unselected");
    expect(inflowReviewSnapshotKey(newer)).not.toBe(
      inflowReviewSnapshotKey(selected[0]),
    );
    expect(
      reconcileInflowSeenBatch(
        [newer, selected[1], selected[2], unselected],
        result.updatedVersions,
      ),
    ).toEqual([
      newer,
      selected[1],
      { ...selected[2], reviewed: true, version: 2 },
      unselected,
    ]);
  });

  it("fills selected queue rows without an unused scrollbar strip or inset accent", () => {
    const css = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
    expect(css).toMatch(
      /\.home-inflow-review__queue ol\s*\{[^}]*overflow-y: auto;[^}]*scrollbar-gutter: auto;/,
    );
    expect(css).toMatch(
      /\.home-inflow-review__queue-item::before\s*\{[^}]*inset: 0 auto 0 0;/,
    );
  });
  it("bounds the desktop grid track so expanded assignment forms scroll instead of clipping", () => {
    const css = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
    expect(css).toMatch(
      /\.home-inflow-review__selection\s*\{[^}]*grid-template-rows: minmax\(0, 1fr\);[^}]*max-height: min\(640px, calc\(100dvh - 160px\)\);/,
    );
    expect(css).toMatch(
      /\.home-inflow\[data-view="calendar"\] \.home-inflow-review__selection\s*\{[^}]*max-height: min\(760px, calc\(100dvh - 160px\)\);/,
    );
    expect(css).toMatch(
      /\.home-inflow-review__detail\s*\{[^}]*min-height: 0;[^}]*overflow-y: auto;/,
    );
  });
  it("separates unread requests and updates without rendering reviewed history on home", () => {
    const markup = renderReview([
      inflow("new"),
      inflow("update", { promotedTaskId: "existing-task" }),
      inflow("read", { reviewed: true }),
    ]);
    expect(markup).toContain(copy.projects.inflowNewTitle);
    expect(markup).toContain(copy.projects.inflowExistingTitle);
    expect(markup).toContain(copy.projects.inflowExistingRowBadge);
    expect(markup).not.toContain(copy.projects.inflowReviewedTitle);
    expect(markup).not.toContain("업무 read");
    expect(markup).toContain('id="home-inflow-queue-title-new"');
    expect(markup).toContain('id="home-inflow-queue-title-existing"');
    expect(markup).toContain('data-has-selection="false"');
    expect(markup.match(/role="tablist"/g)).toHaveLength(1);
    expect(markup.match(/role="tab"/g)).toHaveLength(3);
    expect(markup.match(/role="tabpanel"/g)).toHaveLength(3);
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

  it("keeps questions in 확인 필요 while their existing analysis refreshes", () => {
    for (const analysisStatus of ["refreshing", "stale"] as const) {
      const markup = renderReview([
        inflow("question", {
          analysisStatus,
          analysisClassification: "question",
        }),
      ]);
      expect(markup).toMatch(
        /id="home-inflow-tab-uncertain"[^>]*aria-selected="true"/,
      );
    }
  });
  it("shows an unread owner-authored task in new requests without an external reply", () => {
    const markup = renderReview([
      inflow("owner", {
        sentByOwner: true,
        senderName: "나",
        reviewed: false,
        messages: [
          {
            senderName: "나",
            sentByOwner: true,
            contentText: "토큰 인증 범위를 추가해 주세요.",
            receivedAt: "2026-07-31T01:00:00Z",
          },
        ],
      }),
    ]);
    expect(markup).toMatch(/id="home-inflow-tab-new"[^>]*aria-selected="true"/);
    expect(markup).toContain("업무 owner");
    expect(markup).toContain('data-view="list"');
    expect(markup).not.toContain('<details class="project-inflow__history">');
  });
  it("keeps an uncertain owner-authored request available instead of hiding it as reviewed", () => {
    const markup = renderReview([
      inflow("owner-question", {
        sentByOwner: true,
        senderName: "나",
        reviewed: false,
        analysisClassification: "question",
      }),
    ]);
    expect(markup).toMatch(
      /id="home-inflow-tab-uncertain"[^>]*aria-selected="true"/,
    );
    expect(markup).toContain("업무 owner-question");
    expect(markup).not.toContain('<details class="project-inflow__history">');
  });
  it("keeps uncertain requests in their own review tab without allowing automatic promotion", () => {
    const markup = renderReview([
      inflow("uncertain", { analysisClassification: "noise" }),
    ]);
    expect(markup).toMatch(
      /id="home-inflow-tab-uncertain"[^>]*aria-selected="true"/,
    );
    expect(markup).toContain(copy.projects.inflowUncertainDescription);
    expect(markup).toContain('id="home-inflow-panel-new"');
    expect(markup).toContain(copy.projects.inflowNewEmpty);
  });
  it("shows four rows and offers to reveal the entire unread queue", () => {
    const items = Array.from({ length: 7 }, (_, index) =>
      inflow(`${index + 1}`),
    );
    const markup = renderReview(items);

    expect(markup).toContain('class="home-inflow-review__queue"');
    expect(markup).toContain("업무 4");
    expect(markup).not.toContain("업무 5");
    expect(markup).toContain(copy.home.inboxMore(3));
    expect(visibleHomeInflowItems(items, false)).toHaveLength(4);
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
    expect(renderReview([followUp])).toContain("업무 follow-up");
    expect(renderReview([followUp])).toMatch(
      /id="home-inflow-tab-existing"[^>]*aria-selected="true"/,
    );
    expect(renderReview([promoted, dismissed])).toContain(copy.home.inboxClear);
  });

  it("falls back to a visible conversation when filtering or collapsing removes the selection", () => {
    const items = Array.from({ length: 7 }, (_, index) =>
      inflow(`${index + 1}`),
    );
    const collapsed = visibleHomeInflowItems(items, false);

    expect(resolveHomeInflowSelection(collapsed, "7")).toBe(collapsed[0]);
  });

  it("offers a received-date calendar without mixing existing task updates into new requests", () => {
    const markup = renderReview(
      [
        inflow("new", { receivedAt: new Date().toISOString() }),
        inflow("existing", {
          promotedTaskId: "task",
          receivedAt: new Date().toISOString(),
        }),
      ],
      "calendar",
    );
    expect(markup).toContain('data-view="calendar"');
    expect(markup).toContain(copy.projects.inflowHomeCalendarHint);
    expect(markup).toContain(copy.projects.inflowExistingCalendarScope);
    expect(markup).toContain('aria-labelledby="home-inflow-title-new"');
    expect(markup).toContain('aria-labelledby="home-inflow-title-existing"');
    expect(markup).toContain('aria-controls="home-inflow-panel-existing"');
    expect(markup).toContain('role="tabpanel"');
  });
});

function renderReview(
  items: ProjectInflowItem[],
  initialView: "list" | "calendar" = "list",
): string {
  const props: ComponentProps<typeof HomeInflowReview> = {
    initialView,
    items,
    saving: false,
    onPromote: async () => undefined,
    onDismiss: async () => undefined,
    onMarkSeenMany: async () => ({ failed: [], updatedVersions: new Map() }),
    onRetryAnalysis: async () => undefined,
    onRetryCompletion: async () => undefined,
    onOpenTask: async () => undefined,
  };
  return renderToStaticMarkup(createElement(HomeInflowReview, props));
}
