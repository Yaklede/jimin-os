import { afterEach, describe, expect, it, vi } from "vitest";

import {
  createMeeting,
  decideMeetingAction,
  fetchMeeting,
  fetchMeetings,
  finalizeMeetingRecording,
  reanalyzeMeeting,
  startMeetingRecording,
  updateMeetingAction,
  updateMeetingRecordingNotes,
  updateMeetingTranscript,
  uploadMeetingRecordingChunk,
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

  it("opens meetings created before speaker transcripts were added", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "meeting-legacy",
          title: "기존 회의",
          status: "review_ready",
          participants: ["조지민"],
          topics: [],
          risks: [],
          decisions: [],
          actionItems: [],
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchMeeting("https://os.example", "access", "meeting-legacy"),
    ).resolves.toMatchObject({
      id: "meeting-legacy",
      speakers: [],
      transcriptSegments: [],
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

    await reanalyzeMeeting("https://os.example", "access", "meeting-1", 7);

    expect(fetchMock).toHaveBeenCalledWith(
      "https://os.example/v1/meetings/meeting-1/reanalyze",
      expect.objectContaining({ method: "POST" }),
    );
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      expectedVersion: 7,
    });
  });

  it("saves a corrected transcript with an optimistic version", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ version: 8 }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await updateMeetingTranscript("https://os.example", "access", "meeting-1", {
      expectedVersion: 7,
      speakers: [{ speakerKey: "SPEAKER_00", displayName: "지민", ordinal: 0 }],
      segments: [
        {
          id: "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
          speakerKey: "SPEAKER_00",
          ordinal: 0,
          startsAtMilliseconds: 0,
          endsAtMilliseconds: 1_000,
          text: "회의를 시작할게요.",
        },
      ],
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "https://os.example/v1/meetings/meeting-1/transcript",
      expect.objectContaining({ method: "PUT" }),
    );
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toMatchObject({
      expectedVersion: 7,
      speakers: [{ displayName: "지민" }],
      segments: [
        {
          id: "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
          text: "회의를 시작할게요.",
        },
      ],
    });
  });

  it("rejects a malformed transcript update version", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({}), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      ),
    );

    await expect(
      updateMeetingTranscript("https://os.example", "access", "meeting-1", {
        expectedVersion: 7,
        speakers: [
          {
            speakerKey: "SPEAKER_00",
            displayName: "지민",
            ordinal: 0,
          },
        ],
        segments: [
          {
            id: "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
            speakerKey: "SPEAKER_00",
            ordinal: 0,
            startsAtMilliseconds: 0,
            endsAtMilliseconds: 1_000,
            text: "회의를 시작할게요.",
          },
        ],
      }),
    ).rejects.toMatchObject({ code: "unavailable" });
  });

  it("uploads resumable audio and autosaved notes before finalizing", async () => {
    const fetchMock = vi.fn().mockImplementation(() =>
      Promise.resolve(
        new Response(
          JSON.stringify({
            meeting: { id: "meeting-1", status: "recording" },
            recording: { id: "recording-1", state: "recording" },
          }),
          {
            status: 201,
            headers: { "Content-Type": "application/json" },
          },
        ),
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await startMeetingRecording("https://os.example", "access", {
      title: "주간 회의",
      startedAt: "2026-07-28T06:00:00.000Z",
    });
    await uploadMeetingRecordingChunk(
      "https://os.example",
      "access",
      "recording-1",
      0,
      new Blob([new Uint8Array([1, 2, 3])]),
      "audio/webm;codecs=opus",
    );
    await updateMeetingRecordingNotes(
      "https://os.example",
      "access",
      "recording-1",
      "마감일 확인",
    );
    await finalizeMeetingRecording(
      "https://os.example",
      "access",
      "recording-1",
      {
        mimeType: "audio/webm;codecs=opus",
        durationMilliseconds: 12_000,
      },
    );

    expect(fetchMock.mock.calls[1][0]).toContain("/chunks/0");
    expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({
      mimeType: "audio/webm;codecs=opus",
      audioBase64: "AQID",
    });
    expect(fetchMock.mock.calls[2][0]).toContain("/notes");
    expect(fetchMock.mock.calls[3][0]).toContain("/finalize");
  });
});
