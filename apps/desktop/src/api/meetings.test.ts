import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createMeeting,
  decideMeetingAction,
  fetchMeetings,
  reanalyzeMeeting,
  updateMeetingAction,
  type MeetingActionItem,
} from "./meetings";

afterEach(() => vi.unstubAllGlobals());

describe("meeting API", () => {
  it("queues a transcript for analysis", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "meeting-1", status: "queued" }), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await createMeeting("https://os.example/", "access", {
      title: "출시 회의",
      purpose: "출시 범위를 확정한다.",
      participants: ["조지민", "김경주"],
      transcript: "출시 전에 계약 흐름을 검토하기로 했다.",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "https://os.example/v1/meetings",
      expect.objectContaining({ method: "POST" }),
    );
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({
      title: "출시 회의",
      purpose: "출시 범위를 확정한다.",
      participants: ["조지민", "김경주"],
      transcript: "출시 전에 계약 흐름을 검토하기로 했다.",
      projectId: null,
    });
  });

  it("updates a suggested action before applying it", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "item-1", status: "suggested" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);
    const item = {
      id: "item-1",
      version: 3,
    } as MeetingActionItem;

    await updateMeetingAction(
      "https://os.example",
      "access",
      "meeting-1",
      item,
      {
        title: "계약 검토",
        assigneeName: "김경주",
        priority: 2,
        dueAt: "2026-07-28T09:00:00.000Z",
      },
    );

    expect(fetchMock.mock.calls[0][1]).toMatchObject({ method: "PUT" });
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({
      expectedVersion: 3,
      assigneeName: "김경주",
      priority: 2,
    });
  });

  it("reads and approves review items", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ items: [] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ id: "item-1", status: "applied" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchMeetings("https://os.example", "access"),
    ).resolves.toEqual([]);
    await decideMeetingAction(
      "https://os.example",
      "access",
      "meeting-1",
      "item-1",
      "approve",
    );

    expect(fetchMock.mock.calls[1][0]).toContain("/decisions");
    expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({
      decision: "approve",
    });
  });

  it("explicitly requeues a failed analysis", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "meeting-1", status: "queued" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await reanalyzeMeeting("https://os.example", "access", "meeting-1");

    expect(fetchMock).toHaveBeenCalledWith(
      "https://os.example/v1/meetings/meeting-1/reanalyze",
      expect.objectContaining({ method: "POST" }),
    );
  });
});
