import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchSyncChanges, parseCursorFrame } from "./sync";

afterEach(() => vi.unstubAllGlobals());

describe("cross-device sync API", () => {
  it("loads an ordered invalidation page after the acknowledged cursor", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          items: [
            {
              sequence: "42",
              entityType: "task",
              entityId: "019f68cb-9400-7000-8000-000000000042",
              operation: "upsert",
              entityVersion: 3,
              changedAt: "2026-07-21T10:00:00Z",
            },
          ],
          nextCursor: "42",
          currentCursor: "42",
          hasMore: false,
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    const page = await fetchSyncChanges(
      "https://os.jimin.ai.kr/",
      "access",
      "41",
    );

    expect(page.items[0]?.entityType).toBe("task");
    const requested = new URL(String(fetchMock.mock.calls[0]?.[0]));
    expect(requested.pathname).toBe("/v1/sync/changes");
    expect(requested.searchParams.get("after")).toBe("41");
    expect(requested.searchParams.get("limit")).toBe("200");
  });

  it("parses only explicit cursor events", () => {
    expect(parseCursorFrame('event: cursor\ndata: {"cursor":"81"}')).toBe("81");
    expect(parseCursorFrame('event: snapshot\ndata: {"cursor":"82"}')).toBe(
      undefined,
    );
    expect(parseCursorFrame("event: cursor\ndata: not-json")).toBe(undefined);
  });
});
