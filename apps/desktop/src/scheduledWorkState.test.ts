import { describe, expect, it } from "vitest";
import {
  latestScheduledFailure,
  mergeScheduledRuns,
  parseAssigneeFilter,
} from "./scheduledWorkState";
import type { ScheduledWorkRun } from "./api/scheduledWork";
const run = (
  id: string,
  status: ScheduledWorkRun["status"] = "completed",
): ScheduledWorkRun => ({
  id,
  scheduledWorkId: "rule",
  status,
  kind: "primary",
  reason: null,
  taskIds: [],
  messages: [],
  startedAt: "2026-09-14T00:00:00Z",
  scheduledFor: "2026-09-14T00:00:00Z",
});
describe("scheduled work state", () => {
  it("does not keep an old failure on the home summary after a successful run", () => {
    expect(
      latestScheduledFailure(
        { items: [run("2"), run("1", "failed")], nextCursor: null },
        ["rule"],
      ),
    ).toBeUndefined();
    expect(
      latestScheduledFailure(
        { items: [run("3", "failed"), run("2")], nextCursor: null },
        ["rule"],
      )?.id,
    ).toBe("3");
  });
  it("preserves older loaded history across a background refresh", () => {
    const current = {
      items: Array.from({ length: 70 }, (_, i) => run(String(100 - i))),
      nextCursor: "31",
    };
    const next = mergeScheduledRuns(current, {
      items: [run("101"), ...current.items.slice(0, 49)],
      nextCursor: "52",
    });
    expect(next.items).toHaveLength(71);
    expect(next.nextCursor).toBe("31");
  });
  it("normalizes comma-separated names without duplicate recipients", () => {
    expect(parseAssigneeFilter(" 홍길동, 이담당, 홍길동, ")).toEqual([
      "홍길동",
      "이담당",
    ]);
  });
});
