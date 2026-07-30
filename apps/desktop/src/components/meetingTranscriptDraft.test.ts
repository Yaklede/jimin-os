import { describe, expect, it } from "vitest";

import { type MeetingDetail } from "../api/meetings";
import {
  addMeetingTranscriptSpeaker,
  applyMeetingTranscriptDraft,
  applyMeetingTranscriptUpdateToDetail,
  createMeetingTranscriptDraft,
  createMeetingTranscriptDraftState,
  flushMeetingTranscriptAutosave,
  markMeetingTranscriptDraftSaved,
  meetingTranscriptDraftIsDirty,
  meetingTranscriptDraftIsValid,
  meetingTranscriptDraftToInput,
  mergeMeetingTranscriptSegment,
  renameMeetingTranscriptSpeaker,
  splitMeetingTranscriptSegment,
  undoMeetingTranscriptDraft,
  updateMeetingTranscriptSegment,
} from "./meetingTranscriptDraft";

const speakers = [
  {
    id: "speaker-1",
    meetingId: "meeting-1",
    speakerKey: "SPEAKER_00",
    displayName: null,
    ordinal: 0,
  },
  {
    id: "speaker-2",
    meetingId: "meeting-1",
    speakerKey: "SPEAKER_01",
    displayName: "김경주",
    ordinal: 1,
  },
];

const segments = [
  {
    id: "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
    meetingId: "meeting-1",
    speakerId: "speaker-1",
    speakerKey: "SPEAKER_00",
    speakerName: null,
    ordinal: 0,
    startsAtMilliseconds: 0,
    endsAtMilliseconds: 4_000,
    text: "첫 번째 발언입니다.",
    confidence: 90,
    isFinal: true,
  },
  {
    id: "019f5ce8-b832-7ab1-8fe8-4dd8958d676a",
    meetingId: "meeting-1",
    speakerId: "speaker-1",
    speakerKey: "SPEAKER_00",
    speakerName: null,
    ordinal: 1,
    startsAtMilliseconds: 4_000,
    endsAtMilliseconds: 8_000,
    text: "두 번째 발언입니다.",
    confidence: 90,
    isFinal: true,
  },
];

describe("meeting transcript draft", () => {
  it("flushes a follow-up edit after an in-flight autosave finishes", async () => {
    let resolveFirstSave: (version: number) => void = () => undefined;
    const firstSave = new Promise<number>((resolve) => {
      resolveFirstSave = resolve;
    });
    let pendingSave: Promise<number> | null = firstSave;
    let dirty = true;
    let followUpSaveCount = 0;

    const flushing = flushMeetingTranscriptAutosave({
      pending: () => pendingSave,
      dirty: () => dirty,
      saveOnce: async () => {
        followUpSaveCount += 1;
        dirty = false;
        return 2;
      },
    });

    pendingSave = null;
    resolveFirstSave(1);

    await expect(flushing).resolves.toBe(2);
    expect(followUpSaveCount).toBe(1);
  });

  it("renames speakers and serializes reassigned, edited segments", () => {
    let draft = createMeetingTranscriptDraft(speakers, segments);
    draft = renameMeetingTranscriptSpeaker(draft, "SPEAKER_00", "조지민");
    draft = updateMeetingTranscriptSegment(
      draft,
      "019f5ce8-b832-7ab1-8fe8-4dd8958d676a",
      {
        speakerKey: "SPEAKER_01",
        text: "  수정한 두 번째 발언입니다.  ",
      },
    );

    expect(meetingTranscriptDraftToInput(draft, 4)).toMatchObject({
      expectedVersion: 4,
      speakers: [
        { speakerKey: "SPEAKER_00", displayName: "조지민", ordinal: 0 },
        { speakerKey: "SPEAKER_01", displayName: "김경주", ordinal: 1 },
      ],
      segments: [
        {
          id: "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
          speakerKey: "SPEAKER_00",
          ordinal: 0,
        },
        {
          id: "019f5ce8-b832-7ab1-8fe8-4dd8958d676a",
          speakerKey: "SPEAKER_01",
          ordinal: 1,
          text: "수정한 두 번째 발언입니다.",
        },
      ],
    });
  });

  it("adds a manual speaker so an incorrectly grouped segment can be reassigned", () => {
    let draft = createMeetingTranscriptDraft(speakers.slice(0, 1), segments);
    draft = addMeetingTranscriptSpeaker(
      draft,
      "MANUAL_019f5ce8b8327ab28fe84dd8958d676a",
      "주홍석",
    );
    draft = updateMeetingTranscriptSegment(
      draft,
      "019f5ce8-b832-7ab1-8fe8-4dd8958d676a",
      {
        speakerKey: "MANUAL_019f5ce8b8327ab28fe84dd8958d676a",
      },
    );

    expect(meetingTranscriptDraftIsValid(draft)).toBe(true);
    expect(meetingTranscriptDraftToInput(draft, 4)).toMatchObject({
      speakers: [
        { speakerKey: "SPEAKER_00", ordinal: 0 },
        {
          speakerKey: "MANUAL_019f5ce8b8327ab28fe84dd8958d676a",
          displayName: "주홍석",
          ordinal: 1,
        },
      ],
      segments: [
        { speakerKey: "SPEAKER_00" },
        { speakerKey: "MANUAL_019f5ce8b8327ab28fe84dd8958d676a" },
      ],
    });
  });

  it("splits at the cursor and merges adjacent segments from one speaker", () => {
    const draft = createMeetingTranscriptDraft(speakers, segments.slice(0, 1));
    const split = splitMeetingTranscriptSegment(
      draft,
      "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
      7,
      "019f5ce8-b832-7ab2-8fe8-4dd8958d676a",
    );

    expect(split.segments).toHaveLength(2);
    expect(split.segments[0].text).toBe("첫 번째 발언");
    expect(split.segments[1].text).toBe("입니다.");
    expect(split.segments[0].endsAtMilliseconds).toBe(
      split.segments[1].startsAtMilliseconds,
    );

    const merged = mergeMeetingTranscriptSegment(
      split,
      "019f5ce8-b832-7ab2-8fe8-4dd8958d676a",
      "previous",
    );
    expect(merged.segments).toHaveLength(1);
    expect(merged.segments[0].text).toBe("첫 번째 발언 입니다.");
  });

  it("keeps stable segment ids after autosave", () => {
    const initial = createMeetingTranscriptDraft(speakers, segments);
    let state = createMeetingTranscriptDraftState(initial);
    const submitted = renameMeetingTranscriptSpeaker(
      state.present,
      "SPEAKER_00",
      "조지민",
    );
    state = applyMeetingTranscriptDraft(state, submitted);
    const saved = createMeetingTranscriptDraft(
      [{ ...speakers[0], displayName: "조지민" }, speakers[1]],
      segments,
    );

    state = markMeetingTranscriptDraftSaved(state, submitted, saved);

    expect(state.present.segments.map((segment) => segment.localId)).toEqual([
      "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
      "019f5ce8-b832-7ab1-8fe8-4dd8958d676a",
    ]);
    expect(meetingTranscriptDraftIsDirty(state)).toBe(false);
  });

  it("keeps edits made while an earlier autosave is in flight", () => {
    let state = createMeetingTranscriptDraftState(
      createMeetingTranscriptDraft(speakers, segments),
    );
    const submitted = renameMeetingTranscriptSpeaker(
      state.present,
      "SPEAKER_00",
      "조지민",
    );
    state = applyMeetingTranscriptDraft(state, submitted);
    state = applyMeetingTranscriptDraft(
      state,
      updateMeetingTranscriptSegment(
        state.present,
        "019f5ce8-b832-7ab1-8fe8-4dd8958d676a",
        { text: "저장 중에 다시 고친 발언입니다." },
      ),
    );

    state = markMeetingTranscriptDraftSaved(state, submitted, submitted);

    expect(state.present.segments[1].text).toBe(
      "저장 중에 다시 고친 발언입니다.",
    );
    expect(meetingTranscriptDraftIsDirty(state)).toBe(true);
  });

  it("applies a committed snapshot locally without losing segment metadata", () => {
    const detail = {
      id: "019f5ce8-b832-7aa0-8fe8-4dd8958d676a",
      workspaceId: null,
      projectId: null,
      projectTitle: null,
      title: "운영 회의",
      purpose: null,
      participants: [],
      transcript: "이전 회의록",
      startedAt: null,
      durationSeconds: 10,
      status: "review_ready",
      summary: "이전 요약",
      topics: [],
      risks: [],
      followUp: null,
      analyzedAt: "2026-07-29T00:00:00.000Z",
      createdAt: "2026-07-29T00:00:00.000Z",
      updatedAt: "2026-07-29T00:00:00.000Z",
      version: 4,
      recording: null,
      speakers,
      transcriptSegments: [
        { ...segments[0], confidence: 82, isFinal: false },
        segments[1],
      ],
      decisions: [],
      actionItems: [],
    } satisfies MeetingDetail;
    let draft = createMeetingTranscriptDraft(
      detail.speakers,
      detail.transcriptSegments,
    );
    draft = renameMeetingTranscriptSpeaker(draft, "SPEAKER_00", "조지민");
    draft = splitMeetingTranscriptSegment(
      draft,
      "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
      7,
      "019f5ce8-b832-7ab2-8fe8-4dd8958d676a",
    );
    draft = updateMeetingTranscriptSegment(
      draft,
      "019f5ce8-b832-7ab1-8fe8-4dd8958d676a",
      { speakerKey: "SPEAKER_01" },
    );

    const updated = applyMeetingTranscriptUpdateToDetail(
      detail,
      meetingTranscriptDraftToInput(draft, detail.version),
      5,
    );

    expect(updated).toMatchObject({
      version: 5,
      analyzedAt: null,
      speakers: [{ displayName: "조지민" }, { displayName: "김경주" }],
    });
    expect(updated.transcriptSegments).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: "019f5ce8-b832-7ab0-8fe8-4dd8958d676a",
          confidence: 82,
          isFinal: false,
        }),
        expect.objectContaining({
          id: "019f5ce8-b832-7ab2-8fe8-4dd8958d676a",
          confidence: null,
          isFinal: true,
        }),
        expect.objectContaining({
          id: "019f5ce8-b832-7ab1-8fe8-4dd8958d676a",
          speakerKey: "SPEAKER_01",
          speakerName: "김경주",
        }),
      ]),
    );
    expect(updated.transcript).toContain("[00:00] 조지민:");
    expect(updated.transcript).toContain("김경주: 두 번째 발언입니다.");
  });

  it("undoes the latest local change", () => {
    const state = createMeetingTranscriptDraftState(
      createMeetingTranscriptDraft(speakers, segments),
    );
    const changed = applyMeetingTranscriptDraft(
      state,
      renameMeetingTranscriptSpeaker(state.present, "SPEAKER_00", "조지민"),
    );

    expect(
      undoMeetingTranscriptDraft(changed).present.speakers[0].displayName,
    ).toBe("");
  });

  it("matches server limits for names, segments, timestamps and transcript size", () => {
    const valid = createMeetingTranscriptDraft(speakers, segments);
    expect(meetingTranscriptDraftIsValid(valid)).toBe(true);
    expect(
      meetingTranscriptDraftIsValid({
        ...valid,
        speakers: [
          { ...valid.speakers[0], displayName: "가".repeat(121) },
          valid.speakers[1],
        ],
      }),
    ).toBe(false);
    expect(
      meetingTranscriptDraftIsValid({
        ...valid,
        segments: [
          {
            ...valid.segments[0],
            endsAtMilliseconds: 43_200_001,
          },
        ],
      }),
    ).toBe(false);
    expect(
      meetingTranscriptDraftIsValid({
        ...valid,
        segments: [
          { ...valid.segments[0], startsAtMilliseconds: 5_000 },
          { ...valid.segments[1], startsAtMilliseconds: 4_000 },
        ],
      }),
    ).toBe(false);
    expect(
      meetingTranscriptDraftIsValid({
        ...valid,
        segments: [{ ...valid.segments[0], localId: "temporary-segment" }],
      }),
    ).toBe(false);

    const oversized = {
      speakers: valid.speakers.slice(0, 1),
      segments: Array.from({ length: 16 }, (_, index) => ({
        ...valid.segments[0],
        localId: `019f5ce8-b832-7ab3-8fe8-${String(index).padStart(12, "0")}`,
        ordinal: index,
        startsAtMilliseconds: index * 1_000,
        endsAtMilliseconds: index * 1_000 + 500,
        text: "가".repeat(8_000),
      })),
    };
    expect(meetingTranscriptDraftIsValid(oversized)).toBe(false);
  });
});
