import { describe, it, expect, vi } from "vitest";
import {
  DeckActionDispatcher,
  deckRange,
  normalizeDeckSnapshot,
  loadDeckSnapshot,
} from "./open-deck-bridge";
const id = "019f68cb-9400-7000-8000-000000000001";
describe("Open Deck companion boundary", () => {
  it("shares minimal schedule/request/task fields without tokens, original messages or notes", () => {
    const now = new Date("2026-10-21T01:00:00Z");
    const result = normalizeDeckSnapshot(
      {
        accessToken: "private",
        inflow: [
          {
            id,
            status: "pending",
            reviewed: false,
            analysisStatus: "ready",
            analysisClassification: "new_task",
            suggestedTaskTitle: "Review",
            contentText: "PRIVATE_MESSAGE",
            referenceLinks: ["PRIVATE_LINK"],
            receivedAt: now.toISOString(),
          },
        ],
        tasks: [
          {
            id,
            status: "open",
            title: "Task",
            dueAt: "2026-10-21T08:00:00Z",
            notes: "PRIVATE_NOTES",
          },
        ],
      },
      [
        {
          id,
          title: "Meeting",
          startsAt: "2026-10-21T03:00:00Z",
          endsAt: "2026-10-21T04:00:00Z",
          notes: "PRIVATE_NOTES",
          status: "confirmed",
        },
      ],
      [],
      now,
    );
    expect(result.counts.requests).toBe(1);
    expect(result.counts.today).toBe(1);
    expect(result.events[0].title).toBe("Meeting");
    expect(JSON.stringify(result)).not.toMatch(
      /PRIVATE_|accessToken|notes|contentText|referenceLinks/,
    );
  });
  it("uses the Korean month boundary across UTC dates and excludes cancelled entries", () => {
    expect(deckRange(new Date("2026-09-30T16:00:00Z")).from).toBe(
      "2026-09-30T15:00:00.000Z",
    );
    expect(
      normalizeDeckSnapshot(
        {},
        [
          {
            id,
            status: "cancelled",
            startsAt: "2026-10-21T00:00:00Z",
            endsAt: "2026-10-21T01:00:00Z",
          },
        ],
        [],
      ).events,
    ).toEqual([]);
  });
  it("makes only read requests and reports an expired session to the existing refresh coordinator", async () => {
    const seen: string[] = [];
    await expect(
      loadDeckSnapshot(
        "https://os.example",
        "secret",
        new Date(),
        async (input, init) => {
          seen.push(String(input));
          expect(init?.method).toBeUndefined();
          return new Response("{}", { status: 401 });
        },
      ),
    ).rejects.toMatchObject({ code: "unauthorized" });
    expect(seen).toHaveLength(3);
  });
});

describe("bounded companion actions", () => {
  it("aborts timed-out work once and prevents late navigation", async () => {
    vi.useFakeTimers();
    let release!: () => void;
    let moved = false;
    const completed: string[] = [];
    const dispatcher = new DeckActionDispatcher(
      async (_action, signal) => {
        await new Promise<void>((resolve) => (release = resolve));
        if (!signal.aborted) moved = true;
      },
      (_id, error) => completed.push(error || "ok"),
      10,
    );
    dispatcher.run("one", { operation: "navigate", view: "calendar" });
    await vi.advanceTimersByTimeAsync(11);
    expect(completed).toEqual(["ACTION_TIMEOUT"]);
    release();
    await vi.runAllTimersAsync();
    expect(moved).toBe(false);
    expect(completed).toHaveLength(1);
    dispatcher.dispose();
    vi.useRealTimers();
  });
  it("caps unresolved workers even after their acknowledgement times out", async () => {
    vi.useFakeTimers();
    const codes: string[] = [];
    const dispatcher = new DeckActionDispatcher(
      () => new Promise(() => {}),
      (_id, error) => codes.push(error || "ok"),
      10,
    );
    for (let i = 0; i < 4; i++)
      dispatcher.run(String(i), { operation: "refresh" });
    await vi.advanceTimersByTimeAsync(11);
    dispatcher.run("overflow", { operation: "refresh" });
    expect(codes.filter((x) => x === "ACTION_TIMEOUT")).toHaveLength(4);
    expect(codes.at(-1)).toBe("BUSY");
    dispatcher.dispose();
    vi.useRealTimers();
  });
  it("cancels outstanding work on disconnect and never acknowledges after disposal", async () => {
    let release!: () => void;
    let signal: AbortSignal | undefined;
    const finish = vi.fn();
    const dispatcher = new DeckActionDispatcher(async (_action, s) => {
      signal = s;
      await new Promise<void>((r) => (release = r));
    }, finish);
    dispatcher.run("one", { operation: "refresh" });
    await Promise.resolve();
    dispatcher.dispose();
    expect(signal?.aborted).toBe(true);
    release();
    await Promise.resolve();
    await Promise.resolve();
    expect(finish).not.toHaveBeenCalled();
  });
});
