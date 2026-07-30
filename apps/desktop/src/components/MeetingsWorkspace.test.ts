import { describe, expect, it } from "vitest";

import { type MeetingTranscriptSegment } from "../api/meetings";
import {
  groupTranscriptSegments,
  meetingRecordingAudioConstraints,
} from "./MeetingsWorkspace";
import { meetingTranscriptQuality } from "./MeetingTranscriptPanel";

function segment(
  id: string,
  speakerId: string,
  startsAtMilliseconds: number,
  endsAtMilliseconds: number,
  text: string,
): MeetingTranscriptSegment {
  return {
    id,
    meetingId: "meeting-1",
    speakerId,
    speakerKey: speakerId,
    speakerName: null,
    ordinal: Number(id.replace(/\D/g, "")) || 0,
    startsAtMilliseconds,
    endsAtMilliseconds,
    text,
    confidence: 0.9,
    isFinal: true,
  };
}

describe("meeting transcript grouping", () => {
  it("joins nearby consecutive statements from the same speaker", () => {
    const groups = groupTranscriptSegments([
      segment("1", "speaker-a", 0, 2_000, "첫 번째 문장입니다."),
      segment("2", "speaker-a", 3_000, 5_000, "내용을 이어서 말합니다."),
      segment("3", "speaker-b", 6_000, 8_000, "다른 사람이 답합니다."),
    ]);

    expect(groups).toHaveLength(2);
    expect(groups[0]).toMatchObject({
      speakerId: "speaker-a",
      segmentCount: 2,
      text: "첫 번째 문장입니다. 내용을 이어서 말합니다.",
      endsAtMilliseconds: 5_000,
    });
    expect(groups[1]).toMatchObject({
      speakerId: "speaker-b",
      segmentCount: 1,
    });
  });

  it("keeps distant statements and long turns visually separated", () => {
    const groups = groupTranscriptSegments([
      segment("1", "speaker-a", 0, 1_000, "하나"),
      segment("2", "speaker-a", 2_000, 3_000, "둘"),
      segment("3", "speaker-a", 4_000, 5_000, "셋"),
      segment("4", "speaker-a", 6_000, 7_000, "넷"),
      segment("5", "speaker-a", 13_000, 14_000, "시간이 지난 뒤"),
    ]);

    expect(groups.map((group) => group.segmentCount)).toEqual([3, 1, 1]);
    expect(groups[2].text).toBe("시간이 지난 뒤");
  });
});

describe("meeting recording audio", () => {
  it("preserves voice characteristics used for speaker separation", () => {
    expect(meetingRecordingAudioConstraints()).toEqual({
      echoCancellation: false,
      noiseSuppression: false,
      autoGainControl: false,
      channelCount: 1,
    });
  });
});

describe("meeting transcript quality", () => {
  it("asks for review when multiple participants collapse into one speaker", () => {
    expect(
      meetingTranscriptQuality(
        ["조지민", "주홍석", " 조지민 "],
        [segment("1", "speaker-a", 0, 2_000, "첫 발언")],
      ),
    ).toEqual({
      needsReview: true,
      participantCount: 2,
      speakerCount: 1,
    });
  });

  it("does not imply that every listed participant spoke", () => {
    expect(
      meetingTranscriptQuality(
        ["조지민", "주홍석"],
        [
          segment("1", "speaker-a", 0, 2_000, "첫 발언"),
          segment("2", "speaker-b", 3_000, 5_000, "두 번째 발언"),
        ],
      ).needsReview,
    ).toBe(false);
  });

  it("does not offer transcript editing before any speech is available", () => {
    expect(meetingTranscriptQuality(["조지민", "주홍석"], []).needsReview).toBe(
      false,
    );
  });
});
