import { ChevronDown, CircleAlert, FileAudio, Pencil } from "lucide-react";
import { useEffect, useState } from "react";

import {
  type MeetingDetail,
  type MeetingSpeaker,
  type MeetingTranscriptSegment,
  type MeetingTranscriptUpdateInput,
  type MeetingTranscriptUpdateResult,
} from "../api/meetings";
import { copy } from "../copy";
import { MeetingTranscriptEditor } from "./MeetingTranscriptEditor";

export type MeetingTranscriptGroup = {
  id: string;
  speakerId: string;
  speakerKey: string;
  speakerName: string | null;
  startsAtMilliseconds: number;
  endsAtMilliseconds: number;
  text: string;
  segmentCount: number;
};

type MeetingTranscriptPanelProps = {
  meetingId: string;
  meetingVersion: number;
  notes: string | undefined;
  participants: string[];
  speakers: MeetingSpeaker[];
  segments: MeetingTranscriptSegment[];
  onSave(
    input: MeetingTranscriptUpdateInput,
  ): Promise<MeetingTranscriptUpdateResult>;
  onReload(): Promise<MeetingDetail>;
  onReanalyze(expectedVersion: number): Promise<void>;
  onDirtyChange(dirty: boolean): void;
};

export function groupTranscriptSegments(
  segments: MeetingTranscriptSegment[],
): MeetingTranscriptGroup[] {
  return segments.reduce<MeetingTranscriptGroup[]>((groups, segment) => {
    const previous = groups.at(-1);
    const closeToPrevious =
      previous &&
      segment.startsAtMilliseconds - previous.endsAtMilliseconds <= 5_000;
    if (
      previous &&
      previous.speakerId === segment.speakerId &&
      previous.segmentCount < 3 &&
      closeToPrevious
    ) {
      previous.text = `${previous.text} ${segment.text}`.trim();
      previous.endsAtMilliseconds = segment.endsAtMilliseconds;
      previous.segmentCount += 1;
      return groups;
    }
    groups.push({
      id: segment.id,
      speakerId: segment.speakerId,
      speakerKey: segment.speakerKey,
      speakerName: segment.speakerName,
      startsAtMilliseconds: segment.startsAtMilliseconds,
      endsAtMilliseconds: segment.endsAtMilliseconds,
      text: segment.text,
      segmentCount: 1,
    });
    return groups;
  }, []);
}

export function MeetingTranscriptPanel({
  meetingId,
  meetingVersion,
  notes,
  participants,
  speakers,
  segments,
  onSave,
  onReload,
  onReanalyze,
  onDirtyChange,
}: MeetingTranscriptPanelProps) {
  const [expanded, setExpanded] = useState(false);
  const [editing, setEditing] = useState(false);
  const groups = groupTranscriptSegments(segments);
  const previewLimit = 6;
  const visibleGroups = expanded ? groups : groups.slice(0, previewLimit);
  const remainingCount = Math.max(0, groups.length - previewLimit);
  const speakerById = new Map(speakers.map((speaker) => [speaker.id, speaker]));
  const quality = meetingTranscriptQuality(participants, segments);

  useEffect(() => {
    setExpanded(false);
    setEditing(false);
    onDirtyChange(false);
  }, [meetingId, onDirtyChange]);

  function speakerLabel(
    speaker: MeetingSpeaker | undefined,
    group?: MeetingTranscriptGroup,
  ) {
    if (speaker?.displayName) return speaker.displayName;
    if (group?.speakerName) return group.speakerName;
    const ordinal =
      speaker?.ordinal ??
      Math.max(
        0,
        speakers.findIndex((candidate) => candidate.id === group?.speakerId),
      );
    return copy.meetings.unnamedSpeaker(ordinal + 1);
  }

  if (editing) {
    return (
      <MeetingTranscriptEditor
        meetingId={meetingId}
        meetingVersion={meetingVersion}
        participants={participants}
        speakers={speakers}
        segments={segments}
        onSave={onSave}
        onReload={onReload}
        onReanalyze={onReanalyze}
        onDirtyChange={onDirtyChange}
        onClose={() => {
          setEditing(false);
          onDirtyChange(false);
        }}
      />
    );
  }

  return (
    <section
      className="meeting-transcript-panel"
      aria-labelledby={`meeting-transcript-${meetingId}`}
    >
      <header className="meeting-transcript-panel__header">
        <div className="meeting-transcript-panel__title">
          <span aria-hidden="true">
            <FileAudio />
          </span>
          <div>
            <h3 id={`meeting-transcript-${meetingId}`}>
              {copy.meetings.transcriptTimeline}
            </h3>
            <p>
              {copy.meetings.speakerAndSegmentCount(
                speakers.length,
                segments.length,
              )}
            </p>
          </div>
        </div>
        <div className="meeting-transcript-panel__header-actions">
          {speakers.length > 0 && (
            <ul
              className="meeting-transcript-panel__speakers"
              aria-label={copy.meetings.speakerLegend}
            >
              {speakers.map((speaker) => (
                <li key={speaker.id} data-speaker-tone={speaker.ordinal % 4}>
                  <span aria-hidden="true">
                    {speakerInitial(speakerLabel(speaker), speaker.ordinal)}
                  </span>
                  {speakerLabel(speaker)}
                </li>
              ))}
            </ul>
          )}
          {segments.length > 0 && (
            <button
              className="secondary-button meeting-transcript-panel__edit focus-visible-control"
              type="button"
              onClick={() => setEditing(true)}
            >
              <Pencil aria-hidden="true" />
              {copy.meetings.editTranscript}
            </button>
          )}
        </div>
      </header>

      {quality.needsReview && (
        <section className="meeting-transcript-panel__quality" role="status">
          <CircleAlert aria-hidden="true" />
          <div>
            <strong>{copy.meetings.speakerReviewTitle}</strong>
            <p>
              {copy.meetings.speakerReviewDescription(
                quality.participantCount,
                quality.speakerCount,
              )}
            </p>
          </div>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            onClick={() => setEditing(true)}
          >
            <Pencil aria-hidden="true" />
            {copy.meetings.reviewSpeakers}
          </button>
        </section>
      )}

      {notes && (
        <section className="meeting-transcript-panel__notes">
          <strong>{copy.meetings.recordedNotes}</strong>
          <p>{notes}</p>
        </section>
      )}

      {visibleGroups.length > 0 && (
        <ol className="meeting-transcript-panel__segments">
          {visibleGroups.map((group) => {
            const speaker = speakerById.get(group.speakerId);
            const speakerOrdinal =
              speaker?.ordinal ??
              Math.max(
                0,
                speakers.findIndex(
                  (candidate) => candidate.id === group.speakerId,
                ),
              );
            const label = speakerLabel(speaker, group);
            return (
              <li key={group.id} data-speaker-tone={speakerOrdinal % 4}>
                <span
                  className="meeting-transcript-panel__avatar"
                  aria-hidden="true"
                >
                  {speakerInitial(label, speakerOrdinal)}
                </span>
                <div>
                  <header>
                    <strong>{label}</strong>
                    <time>{segmentTimestamp(group.startsAtMilliseconds)}</time>
                  </header>
                  <p>{group.text}</p>
                </div>
              </li>
            );
          })}
        </ol>
      )}

      {remainingCount > 0 && (
        <button
          className="meeting-transcript-panel__toggle focus-visible-control"
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded((current) => !current)}
        >
          {expanded
            ? copy.meetings.collapseTranscript
            : copy.meetings.moreTranscript(remainingCount)}
          <ChevronDown aria-hidden="true" />
        </button>
      )}
    </section>
  );
}

export function meetingTranscriptQuality(
  participants: string[],
  segments: MeetingTranscriptSegment[],
): {
  needsReview: boolean;
  participantCount: number;
  speakerCount: number;
} {
  const participantCount = new Set(
    participants
      .map((participant) => participant.trim().toLocaleLowerCase("ko-KR"))
      .filter(Boolean),
  ).size;
  const speakerCount = new Set(
    segments.map((segment) => segment.speakerKey.trim()).filter(Boolean),
  ).size;
  return {
    needsReview: participantCount >= 2 && speakerCount === 1,
    participantCount,
    speakerCount,
  };
}

function speakerInitial(label: string, ordinal: number): string {
  return label.trim().slice(0, 1) || String(ordinal + 1);
}

function segmentTimestamp(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1_000));
  const hours = Math.floor(totalSeconds / 3_600);
  const minutes = Math.floor((totalSeconds % 3_600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) {
    return [hours, minutes, seconds]
      .map((part) => String(part).padStart(2, "0"))
      .join(":");
  }
  return [minutes, seconds]
    .map((part) => String(part).padStart(2, "0"))
    .join(":");
}
