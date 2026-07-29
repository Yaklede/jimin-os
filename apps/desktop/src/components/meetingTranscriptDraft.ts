import {
  type MeetingDetail,
  type MeetingSpeaker,
  type MeetingTranscriptSegment,
  type MeetingTranscriptUpdateInput,
} from "../api/meetings";
import { isUuidV7 } from "../uuid";

export type MeetingTranscriptDraftSpeaker = {
  speakerKey: string;
  displayName: string;
  ordinal: number;
};

export type MeetingTranscriptDraftSegment = {
  localId: string;
  speakerKey: string;
  ordinal: number;
  startsAtMilliseconds: number;
  endsAtMilliseconds: number;
  text: string;
};

export type MeetingTranscriptDraft = {
  speakers: MeetingTranscriptDraftSpeaker[];
  segments: MeetingTranscriptDraftSegment[];
};

export type MeetingTranscriptDraftState = {
  past: MeetingTranscriptDraft[];
  present: MeetingTranscriptDraft;
  future: MeetingTranscriptDraft[];
  savedSignature: string;
};

type MeetingTranscriptAutosaveFlushOptions<T> = {
  pending(): Promise<T> | null;
  dirty(): boolean;
  saveOnce(): Promise<T | undefined>;
  maxAttempts?: number;
};

export async function flushMeetingTranscriptAutosave<T>({
  pending,
  dirty,
  saveOnce,
  maxAttempts = 3,
}: MeetingTranscriptAutosaveFlushOptions<T>): Promise<T | undefined> {
  let saved: T | undefined;
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const inFlight = pending();
    if (inFlight) {
      saved = await inFlight;
    }
    if (!dirty()) break;
    saved = await saveOnce();
  }
  return saved;
}

const MAX_UNDO_STEPS = 40;
const MAX_SPEAKER_KEY_CHARS = 80;
const MAX_SPEAKER_NAME_CHARS = 120;
const MAX_SEGMENT_TEXT_CHARS = 8_000;
const MAX_TRANSCRIPT_CHARS = 120_000;
const MAX_MEETING_MILLISECONDS = 43_200_000;

export function createMeetingTranscriptDraft(
  speakers: MeetingSpeaker[],
  segments: MeetingTranscriptSegment[],
): MeetingTranscriptDraft {
  return normalizeDraft({
    speakers: speakers.map((speaker) => ({
      speakerKey: speaker.speakerKey,
      displayName: speaker.displayName ?? "",
      ordinal: speaker.ordinal,
    })),
    segments: segments.map((segment) => ({
      localId: segment.id,
      speakerKey: segment.speakerKey,
      ordinal: segment.ordinal,
      startsAtMilliseconds: segment.startsAtMilliseconds,
      endsAtMilliseconds: segment.endsAtMilliseconds,
      text: segment.text,
    })),
  });
}

export function createMeetingTranscriptDraftState(
  draft: MeetingTranscriptDraft,
): MeetingTranscriptDraftState {
  const present = normalizeDraft(draft);
  return {
    past: [],
    present,
    future: [],
    savedSignature: meetingTranscriptDraftSignature(present),
  };
}

export function applyMeetingTranscriptDraft(
  state: MeetingTranscriptDraftState,
  nextDraft: MeetingTranscriptDraft,
): MeetingTranscriptDraftState {
  const next = normalizeDraft(nextDraft);
  if (
    meetingTranscriptDraftSignature(next) ===
    meetingTranscriptDraftSignature(state.present)
  ) {
    return state;
  }
  return {
    past: [...state.past.slice(-(MAX_UNDO_STEPS - 1)), state.present],
    present: next,
    future: [],
    savedSignature: state.savedSignature,
  };
}

export function undoMeetingTranscriptDraft(
  state: MeetingTranscriptDraftState,
): MeetingTranscriptDraftState {
  const previous = state.past.at(-1);
  if (!previous) return state;
  return {
    past: state.past.slice(0, -1),
    present: previous,
    future: [state.present, ...state.future],
    savedSignature: state.savedSignature,
  };
}

export function markMeetingTranscriptDraftSaved(
  state: MeetingTranscriptDraftState,
  _submittedDraft: MeetingTranscriptDraft,
  savedDraft: MeetingTranscriptDraft,
): MeetingTranscriptDraftState {
  const saved = normalizeDraft(savedDraft);
  return {
    ...state,
    // Keep the active draft in place so a successful autosave never remounts
    // the textarea or loses its caret.
    present: state.present,
    savedSignature: meetingTranscriptDraftSignature(saved),
  };
}

export function meetingTranscriptDraftIsDirty(
  state: MeetingTranscriptDraftState,
): boolean {
  return (
    meetingTranscriptDraftSignature(state.present) !== state.savedSignature
  );
}

export function meetingTranscriptDraftIsValid(
  draft: MeetingTranscriptDraft,
): boolean {
  const speakerKeys = new Set(
    draft.speakers.map((speaker) => speaker.speakerKey),
  );
  const reconstructedTranscriptLength = draft.segments.reduce(
    (length, segment, index) => {
      const speaker = draft.speakers.find(
        (candidate) => candidate.speakerKey === segment.speakerKey,
      );
      const label = speaker?.displayName.trim() || segment.speakerKey;
      const seconds = Math.max(
        0,
        Math.floor(segment.startsAtMilliseconds / 1_000),
      );
      const timestamp = `[${String(Math.floor(seconds / 60)).padStart(
        2,
        "0",
      )}:${String(seconds % 60).padStart(2, "0")}]`;
      // Server reconstruction: "[MM:SS] {speaker}: {text}", joined by "\n".
      return (
        length +
        (index > 0 ? 1 : 0) +
        timestamp.length +
        1 +
        label.length +
        2 +
        segment.text.trim().length
      );
    },
    0,
  );
  return (
    draft.speakers.length > 0 &&
    speakerKeys.size === draft.speakers.length &&
    draft.speakers.every(
      (speaker) =>
        speaker.speakerKey.trim().length >= 1 &&
        speaker.speakerKey.trim().length <= MAX_SPEAKER_KEY_CHARS &&
        speaker.displayName.trim().length <= MAX_SPEAKER_NAME_CHARS,
    ) &&
    draft.segments.length > 0 &&
    draft.segments.every((segment, index) => {
      const previous = draft.segments[index - 1];
      return (
        isUuidV7(segment.localId) &&
        speakerKeys.has(segment.speakerKey) &&
        segment.text.trim().length > 0 &&
        segment.text.trim().length <= MAX_SEGMENT_TEXT_CHARS &&
        segment.startsAtMilliseconds >= 0 &&
        segment.endsAtMilliseconds > segment.startsAtMilliseconds &&
        segment.endsAtMilliseconds <= MAX_MEETING_MILLISECONDS &&
        (!previous ||
          segment.startsAtMilliseconds >= previous.startsAtMilliseconds)
      );
    }) &&
    reconstructedTranscriptLength <= MAX_TRANSCRIPT_CHARS
  );
}

export function renameMeetingTranscriptSpeaker(
  draft: MeetingTranscriptDraft,
  speakerKey: string,
  displayName: string,
): MeetingTranscriptDraft {
  return {
    ...draft,
    speakers: draft.speakers.map((speaker) =>
      speaker.speakerKey === speakerKey ? { ...speaker, displayName } : speaker,
    ),
  };
}

export function updateMeetingTranscriptSegment(
  draft: MeetingTranscriptDraft,
  localId: string,
  change: Partial<Pick<MeetingTranscriptDraftSegment, "speakerKey" | "text">>,
): MeetingTranscriptDraft {
  return {
    ...draft,
    segments: draft.segments.map((segment) =>
      segment.localId === localId ? { ...segment, ...change } : segment,
    ),
  };
}

export function splitMeetingTranscriptSegment(
  draft: MeetingTranscriptDraft,
  localId: string,
  cursor: number,
  newLocalId: string,
): MeetingTranscriptDraft {
  const index = draft.segments.findIndex(
    (segment) => segment.localId === localId,
  );
  if (index < 0) return draft;
  const segment = draft.segments[index];
  const splitAt = Math.max(0, Math.min(segment.text.length, cursor));
  const firstText = segment.text.slice(0, splitAt).trim();
  const secondText = segment.text.slice(splitAt).trim();
  const duration = segment.endsAtMilliseconds - segment.startsAtMilliseconds;
  if (!firstText || !secondText || duration < 2) return draft;

  const proportionalOffset = Math.round(
    duration * (splitAt / Math.max(1, segment.text.length)),
  );
  const boundary = Math.max(
    segment.startsAtMilliseconds + 1,
    Math.min(
      segment.endsAtMilliseconds - 1,
      segment.startsAtMilliseconds + proportionalOffset,
    ),
  );
  const replacement = [
    {
      ...segment,
      text: firstText,
      endsAtMilliseconds: boundary,
    },
    {
      ...segment,
      localId: newLocalId,
      text: secondText,
      startsAtMilliseconds: boundary,
    },
  ];
  return {
    ...draft,
    segments: [
      ...draft.segments.slice(0, index),
      ...replacement,
      ...draft.segments.slice(index + 1),
    ],
  };
}

export function canMergeMeetingTranscriptSegment(
  draft: MeetingTranscriptDraft,
  localId: string,
  direction: "previous" | "next",
): boolean {
  const index = draft.segments.findIndex(
    (segment) => segment.localId === localId,
  );
  const adjacentIndex = direction === "previous" ? index - 1 : index + 1;
  if (
    index < 0 ||
    adjacentIndex < 0 ||
    adjacentIndex >= draft.segments.length
  ) {
    return false;
  }
  return (
    draft.segments[index].speakerKey ===
    draft.segments[adjacentIndex].speakerKey
  );
}

export function mergeMeetingTranscriptSegment(
  draft: MeetingTranscriptDraft,
  localId: string,
  direction: "previous" | "next",
): MeetingTranscriptDraft {
  if (!canMergeMeetingTranscriptSegment(draft, localId, direction)) {
    return draft;
  }
  const index = draft.segments.findIndex(
    (segment) => segment.localId === localId,
  );
  const firstIndex = direction === "previous" ? index - 1 : index;
  const secondIndex = firstIndex + 1;
  const first = draft.segments[firstIndex];
  const second = draft.segments[secondIndex];
  const merged = {
    ...first,
    endsAtMilliseconds: Math.max(
      first.endsAtMilliseconds,
      second.endsAtMilliseconds,
    ),
    text: `${first.text.trim()} ${second.text.trim()}`.trim(),
  };
  return {
    ...draft,
    segments: [
      ...draft.segments.slice(0, firstIndex),
      merged,
      ...draft.segments.slice(secondIndex + 1),
    ],
  };
}

export function meetingTranscriptDraftToInput(
  draft: MeetingTranscriptDraft,
  expectedVersion: number,
): MeetingTranscriptUpdateInput {
  const normalized = normalizeDraft(draft);
  return {
    expectedVersion,
    speakers: normalized.speakers.map((speaker) => ({
      speakerKey: speaker.speakerKey,
      displayName: speaker.displayName.trim() || null,
      ordinal: speaker.ordinal,
    })),
    segments: normalized.segments.map((segment) => ({
      id: segment.localId,
      speakerKey: segment.speakerKey,
      ordinal: segment.ordinal,
      startsAtMilliseconds: segment.startsAtMilliseconds,
      endsAtMilliseconds: segment.endsAtMilliseconds,
      text: segment.text.trim(),
    })),
  };
}

export function applyMeetingTranscriptUpdateToDetail(
  detail: MeetingDetail,
  input: MeetingTranscriptUpdateInput,
  version: number,
): MeetingDetail {
  const existingSpeakers = new Map(
    detail.speakers.map((speaker) => [speaker.speakerKey, speaker]),
  );
  const speakers = input.speakers.map((speaker) => {
    const existing = existingSpeakers.get(speaker.speakerKey);
    return {
      id: existing?.id ?? `${detail.id}:${speaker.speakerKey}`,
      meetingId: detail.id,
      speakerKey: speaker.speakerKey,
      displayName: speaker.displayName,
      ordinal: speaker.ordinal,
    };
  });
  const speakerByKey = new Map(
    speakers.map((speaker) => [speaker.speakerKey, speaker]),
  );
  const existingSegments = new Map(
    detail.transcriptSegments.map((segment) => [segment.id, segment]),
  );
  const transcriptSegments = input.segments.map((segment) => {
    const existing = existingSegments.get(segment.id);
    const speaker = speakerByKey.get(segment.speakerKey);
    return {
      id: segment.id,
      meetingId: detail.id,
      speakerId: speaker?.id ?? existing?.speakerId ?? segment.speakerKey,
      speakerKey: segment.speakerKey,
      speakerName: speaker?.displayName ?? null,
      ordinal: segment.ordinal,
      startsAtMilliseconds: segment.startsAtMilliseconds,
      endsAtMilliseconds: segment.endsAtMilliseconds,
      text: segment.text,
      confidence: existing?.confidence ?? null,
      isFinal: existing?.isFinal ?? true,
    };
  });
  const speakerNames = new Map(
    speakers.map((speaker) => [
      speaker.speakerKey,
      speaker.displayName?.trim() || speaker.speakerKey,
    ]),
  );
  const transcript = transcriptSegments
    .map((segment) => {
      const seconds = Math.max(
        0,
        Math.floor(segment.startsAtMilliseconds / 1_000),
      );
      const timestamp = `${String(Math.floor(seconds / 60)).padStart(
        2,
        "0",
      )}:${String(seconds % 60).padStart(2, "0")}`;
      return `[${timestamp}] ${
        speakerNames.get(segment.speakerKey) ?? segment.speakerKey
      }: ${segment.text.trim()}`;
    })
    .join("\n");

  return {
    ...detail,
    transcript,
    speakers,
    transcriptSegments,
    analyzedAt: null,
    updatedAt: new Date().toISOString(),
    version,
  };
}

export function meetingTranscriptDraftSignature(
  draft: MeetingTranscriptDraft,
): string {
  const normalized = normalizeDraft(draft);
  return JSON.stringify({
    speakers: normalized.speakers.map(
      ({ speakerKey, displayName, ordinal }) => ({
        speakerKey,
        displayName: displayName.trim(),
        ordinal,
      }),
    ),
    segments: normalized.segments.map(
      ({
        speakerKey,
        ordinal,
        startsAtMilliseconds,
        endsAtMilliseconds,
        text,
      }) => ({
        speakerKey,
        ordinal,
        startsAtMilliseconds,
        endsAtMilliseconds,
        text: text.trim(),
      }),
    ),
  });
}

function normalizeDraft(draft: MeetingTranscriptDraft): MeetingTranscriptDraft {
  return {
    speakers: [...draft.speakers]
      .sort(
        (left, right) =>
          left.ordinal - right.ordinal ||
          left.speakerKey.localeCompare(right.speakerKey),
      )
      .map((speaker, ordinal) => ({ ...speaker, ordinal })),
    segments: [...draft.segments]
      .sort(
        (left, right) =>
          left.ordinal - right.ordinal ||
          left.startsAtMilliseconds - right.startsAtMilliseconds,
      )
      .map((segment, ordinal) => ({ ...segment, ordinal })),
  };
}
