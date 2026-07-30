import {
  CircleAlert,
  CornerDownRight,
  CornerUpLeft,
  LoaderCircle,
  Save,
  Scissors,
  Sparkles,
  Undo2,
  UserPlus,
  X,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ChangeEvent,
} from "react";

import {
  type MeetingDetail,
  type MeetingSpeaker,
  type MeetingTranscriptSegment,
  type MeetingTranscriptUpdateInput,
  type MeetingTranscriptUpdateResult,
} from "../api/meetings";
import { PlanningRequestError } from "../api/planning";
import { copy } from "../copy";
import { registerMobileBackHandler } from "../mobileBack";
import { createUuidV7 } from "../uuid";
import {
  addMeetingTranscriptSpeaker,
  applyMeetingTranscriptDraft,
  canMergeMeetingTranscriptSegment,
  createMeetingTranscriptDraft,
  createMeetingTranscriptDraftState,
  flushMeetingTranscriptAutosave,
  markMeetingTranscriptDraftSaved,
  meetingTranscriptDraftIsDirty,
  meetingTranscriptDraftIsValid,
  meetingTranscriptDraftSignature,
  meetingTranscriptDraftToInput,
  mergeMeetingTranscriptSegment,
  renameMeetingTranscriptSpeaker,
  splitMeetingTranscriptSegment,
  undoMeetingTranscriptDraft,
  updateMeetingTranscriptSegment,
  type MeetingTranscriptDraft,
} from "./meetingTranscriptDraft";

type TranscriptSaveState =
  | "idle"
  | "pending"
  | "saving"
  | "reloading"
  | "saved"
  | "invalid"
  | "conflict"
  | "failed"
  | "reanalyze_failed";

type MeetingTranscriptEditorProps = {
  meetingId: string;
  meetingVersion: number;
  participants: string[];
  speakers: MeetingSpeaker[];
  segments: MeetingTranscriptSegment[];
  onSave(
    input: MeetingTranscriptUpdateInput,
  ): Promise<MeetingTranscriptUpdateResult>;
  onReload(): Promise<MeetingDetail>;
  onReanalyze(expectedVersion: number): Promise<void>;
  onClose(): void;
  onDirtyChange(dirty: boolean): void;
};

const AUTOSAVE_DELAY_MILLISECONDS = 900;

export function MeetingTranscriptEditor({
  meetingId,
  meetingVersion,
  participants,
  speakers,
  segments,
  onSave,
  onReload,
  onReanalyze,
  onClose,
  onDirtyChange,
}: MeetingTranscriptEditorProps) {
  const [draftState, setDraftState] = useState(() =>
    createMeetingTranscriptDraftState(
      createMeetingTranscriptDraft(speakers, segments),
    ),
  );
  const [saveState, setSaveState] = useState<TranscriptSaveState>("idle");
  const [reanalyzing, setReanalyzing] = useState(false);
  const [reloading, setReloading] = useState(false);
  const [mobileDialog, setMobileDialog] = useState(
    () => window.matchMedia("(max-width: 720px)").matches,
  );
  const stateRef = useRef(draftState);
  const versionRef = useRef(meetingVersion);
  const savePromiseRef = useRef<Promise<MeetingTranscriptUpdateResult> | null>(
    null,
  );
  const latestSavedResultRef = useRef<
    MeetingTranscriptUpdateResult | undefined
  >(undefined);
  const editorRef = useRef<HTMLElement>(null);
  const restoreFocusRef = useRef(
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );
  const textareaRefs = useRef(new Map<string, HTMLTextAreaElement>());
  const dirty = meetingTranscriptDraftIsDirty(draftState);
  const valid = meetingTranscriptDraftIsValid(draftState.present);
  const busy = reanalyzing || reloading;
  const closeBlocked = busy || saveState === "saving";

  useEffect(() => {
    stateRef.current = draftState;
  }, [draftState]);

  useEffect(() => {
    versionRef.current = Math.max(versionRef.current, meetingVersion);
  }, [meetingVersion]);

  useEffect(() => {
    const next = createMeetingTranscriptDraftState(
      createMeetingTranscriptDraft(speakers, segments),
    );
    stateRef.current = next;
    versionRef.current = meetingVersion;
    latestSavedResultRef.current = undefined;
    setDraftState(next);
    setSaveState("idle");
  }, [meetingId]);

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    const media = window.matchMedia("(max-width: 720px)");
    const update = () => setMobileDialog(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    if (!mobileDialog) return;
    const frame = window.requestAnimationFrame(() => {
      editorRef.current
        ?.querySelector<HTMLElement>("input, textarea, select, button")
        ?.focus();
    });
    const restoreFocus = restoreFocusRef.current;
    return () => {
      window.cancelAnimationFrame(frame);
      restoreFocus?.focus();
    };
  }, [mobileDialog]);

  const changeDraft = useCallback(
    (change: (draft: MeetingTranscriptDraft) => MeetingTranscriptDraft) => {
      setDraftState((current) => {
        const next = applyMeetingTranscriptDraft(
          current,
          change(current.present),
        );
        stateRef.current = next;
        return next;
      });
    },
    [],
  );

  const saveOnce = useCallback(async (): Promise<
    MeetingTranscriptUpdateResult | undefined
  > => {
    if (savePromiseRef.current) return savePromiseRef.current;
    const current = stateRef.current;
    if (!meetingTranscriptDraftIsDirty(current)) {
      return latestSavedResultRef.current;
    }
    if (!meetingTranscriptDraftIsValid(current.present)) {
      setSaveState("invalid");
      return undefined;
    }

    const submittedDraft = current.present;
    setSaveState("saving");
    const operation = (async () => {
      try {
        const saved = await onSave(
          meetingTranscriptDraftToInput(submittedDraft, versionRef.current),
        );
        versionRef.current = saved.version;
        latestSavedResultRef.current = saved;
        const next = markMeetingTranscriptDraftSaved(
          stateRef.current,
          submittedDraft,
          submittedDraft,
        );
        // Keep the ref synchronous with the response. A flush may immediately
        // check for edits made while this request was in flight.
        stateRef.current = next;
        setDraftState(next);
        setSaveState("saved");
        return saved;
      } catch (error) {
        setSaveState(
          error instanceof PlanningRequestError && error.code === "conflict"
            ? "conflict"
            : "failed",
        );
        throw error;
      }
    })();
    savePromiseRef.current = operation;
    void operation.then(
      () => {
        if (savePromiseRef.current === operation) {
          savePromiseRef.current = null;
        }
      },
      () => {
        if (savePromiseRef.current === operation) {
          savePromiseRef.current = null;
        }
      },
    );
    return operation;
  }, [onSave]);

  const flushSave = useCallback(
    async (): Promise<MeetingTranscriptUpdateResult | undefined> =>
      flushMeetingTranscriptAutosave({
        pending: () => savePromiseRef.current,
        dirty: () => meetingTranscriptDraftIsDirty(stateRef.current),
        saveOnce,
      }),
    [saveOnce],
  );

  useEffect(() => {
    if (!dirty) return;
    if (!valid) {
      setSaveState("invalid");
      return;
    }
    setSaveState("pending");
    const timer = window.setTimeout(() => {
      void flushSave().catch(() => undefined);
    }, AUTOSAVE_DELAY_MILLISECONDS);
    return () => window.clearTimeout(timer);
  }, [dirty, draftState.present, draftState.savedSignature, flushSave, valid]);

  const requestClose = useCallback(() => {
    if (closeBlocked) return;
    if (
      meetingTranscriptDraftIsDirty(stateRef.current) &&
      !window.confirm(copy.meetings.transcriptDiscardConfirm)
    ) {
      return;
    }
    onClose();
  }, [closeBlocked, onClose]);

  useEffect(
    () =>
      registerMobileBackHandler(() => {
        requestClose();
        return true;
      }, 120),
    [requestClose],
  );

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (mobileDialog && event.key === "Tab") {
        const controls = Array.from(
          editorRef.current?.querySelectorAll<HTMLElement>(
            'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
          ) ?? [],
        );
        if (controls.length > 0) {
          const first = controls[0];
          const last = controls.at(-1) ?? first;
          if (event.shiftKey && document.activeElement === first) {
            event.preventDefault();
            last.focus();
            return;
          }
          if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault();
            first.focus();
            return;
          }
        }
      }
      if (
        (event.metaKey || event.ctrlKey) &&
        !event.shiftKey &&
        event.key.toLowerCase() === "z"
      ) {
        event.preventDefault();
        setDraftState((current) => {
          const next = undoMeetingTranscriptDraft(current);
          stateRef.current = next;
          return next;
        });
        return;
      }
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      requestClose();
    }
    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, [mobileDialog, requestClose]);

  useEffect(() => {
    function preventUnsavedExit(event: BeforeUnloadEvent) {
      if (!meetingTranscriptDraftIsDirty(stateRef.current)) return;
      event.preventDefault();
    }
    window.addEventListener("beforeunload", preventUnsavedExit);
    return () => window.removeEventListener("beforeunload", preventUnsavedExit);
  }, []);

  async function saveNow() {
    try {
      await flushSave();
    } catch {
      // The visible save state already explains how to recover.
    }
  }

  async function reloadLatest() {
    if (!window.confirm(copy.meetings.transcriptReloadConfirm)) return;
    const startedWith = meetingTranscriptDraftSignature(
      stateRef.current.present,
    );
    setReloading(true);
    setSaveState("reloading");
    try {
      const latest = await onReload();
      if (
        meetingTranscriptDraftSignature(stateRef.current.present) !==
        startedWith
      ) {
        setSaveState("conflict");
        return;
      }
      const next = createMeetingTranscriptDraftState(
        createMeetingTranscriptDraft(
          latest.speakers,
          latest.transcriptSegments,
        ),
      );
      stateRef.current = next;
      versionRef.current = latest.version;
      latestSavedResultRef.current = { version: latest.version };
      setDraftState(next);
      setSaveState("idle");
    } catch (error) {
      setSaveState(
        error instanceof PlanningRequestError && error.code === "conflict"
          ? "conflict"
          : "failed",
      );
    } finally {
      setReloading(false);
    }
  }

  async function reanalyze() {
    if (!meetingTranscriptDraftIsValid(stateRef.current.present)) {
      setSaveState("invalid");
      return;
    }
    if (!window.confirm(copy.meetings.transcriptReanalyzeConfirm)) return;
    setReanalyzing(true);
    try {
      const saved = await flushSave();
      const expectedVersion = saved?.version ?? versionRef.current;
      await onReanalyze(expectedVersion);
      onClose();
    } catch (error) {
      if (error instanceof PlanningRequestError && error.code === "conflict") {
        setSaveState("conflict");
      } else {
        setSaveState(
          meetingTranscriptDraftIsDirty(stateRef.current)
            ? "failed"
            : "reanalyze_failed",
        );
      }
    } finally {
      setReanalyzing(false);
    }
  }

  function undo() {
    setDraftState((current) => {
      const next = undoMeetingTranscriptDraft(current);
      stateRef.current = next;
      return next;
    });
  }

  function addSpeaker() {
    const speakerKey = `MANUAL_${createUuidV7().replaceAll("-", "")}`;
    changeDraft((current) =>
      addMeetingTranscriptSpeaker(
        current,
        speakerKey,
        nextUnassignedParticipant(participants, current),
      ),
    );
  }

  return (
    <section
      ref={editorRef}
      className="meeting-transcript-editor"
      aria-labelledby={`meeting-transcript-editor-${meetingId}`}
      aria-live="off"
      aria-modal={mobileDialog || undefined}
      aria-busy={busy || saveState === "saving"}
      role={mobileDialog ? "dialog" : undefined}
      data-dirty={dirty}
    >
      <header className="meeting-transcript-editor__header">
        <div>
          <span>{copy.meetings.transcriptEditorEyebrow}</span>
          <h3 id={`meeting-transcript-editor-${meetingId}`}>
            {copy.meetings.transcriptEditorTitle}
          </h3>
          <p>{copy.meetings.transcriptEditorDescription}</p>
        </div>
        <button
          className="icon-button focus-visible-control"
          type="button"
          aria-label={copy.meetings.closeTranscriptEditor}
          disabled={closeBlocked}
          onClick={requestClose}
        >
          <X aria-hidden="true" />
        </button>
      </header>

      <div className="meeting-transcript-editor__body">
        <section
          className="meeting-transcript-editor__speakers"
          aria-labelledby={`meeting-speakers-editor-${meetingId}`}
        >
          <div className="meeting-transcript-editor__section-heading">
            <div>
              <h4 id={`meeting-speakers-editor-${meetingId}`}>
                {copy.meetings.speakerNamesTitle}
              </h4>
              <p>{copy.meetings.speakerNamesDescription}</p>
            </div>
            <div className="meeting-transcript-editor__section-actions">
              <span>
                {copy.meetings.count(draftState.present.speakers.length)}
              </span>
              <button
                className="secondary-button focus-visible-control"
                type="button"
                disabled={busy || draftState.present.speakers.length >= 100}
                onClick={addSpeaker}
              >
                <UserPlus aria-hidden="true" />
                {copy.meetings.addSpeaker}
              </button>
            </div>
          </div>
          <div className="meeting-transcript-editor__speaker-grid">
            {draftState.present.speakers.map((speaker) => (
              <label key={speaker.speakerKey}>
                <span>{copy.meetings.unnamedSpeaker(speaker.ordinal + 1)}</span>
                <input
                  type="text"
                  maxLength={120}
                  value={speaker.displayName}
                  placeholder={copy.meetings.speakerNamePlaceholder}
                  disabled={busy}
                  onChange={(event) =>
                    changeDraft((current) =>
                      renameMeetingTranscriptSpeaker(
                        current,
                        speaker.speakerKey,
                        event.target.value,
                      ),
                    )
                  }
                />
              </label>
            ))}
          </div>
        </section>

        <section
          className="meeting-transcript-editor__segments"
          aria-labelledby={`meeting-segments-editor-${meetingId}`}
        >
          <div className="meeting-transcript-editor__section-heading">
            <div>
              <h4 id={`meeting-segments-editor-${meetingId}`}>
                {copy.meetings.segmentEditorTitle}
              </h4>
              <p>{copy.meetings.segmentEditorDescription}</p>
            </div>
            <span>
              {copy.meetings.count(draftState.present.segments.length)}
            </span>
          </div>
          <ol>
            {draftState.present.segments.map((segment) => (
              <li key={segment.localId}>
                <div className="meeting-transcript-editor__segment-meta">
                  <label>
                    <span className="sr-only">
                      {copy.meetings.segmentSpeakerAt(
                        segment.ordinal + 1,
                        segmentRange(
                          segment.startsAtMilliseconds,
                          segment.endsAtMilliseconds,
                        ),
                      )}
                    </span>
                    <select
                      value={segment.speakerKey}
                      disabled={busy}
                      onChange={(event) =>
                        changeDraft((current) =>
                          updateMeetingTranscriptSegment(
                            current,
                            segment.localId,
                            { speakerKey: event.target.value },
                          ),
                        )
                      }
                    >
                      {draftState.present.speakers.map((speaker) => (
                        <option
                          value={speaker.speakerKey}
                          key={speaker.speakerKey}
                        >
                          {speaker.displayName.trim() ||
                            copy.meetings.unnamedSpeaker(speaker.ordinal + 1)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <time>
                    {segmentRange(
                      segment.startsAtMilliseconds,
                      segment.endsAtMilliseconds,
                    )}
                  </time>
                </div>
                <textarea
                  ref={(element) => {
                    if (element) {
                      textareaRefs.current.set(segment.localId, element);
                    } else {
                      textareaRefs.current.delete(segment.localId);
                    }
                  }}
                  maxLength={8_000}
                  value={segment.text}
                  disabled={busy}
                  aria-label={copy.meetings.segmentTextAt(
                    segment.ordinal + 1,
                    segmentRange(
                      segment.startsAtMilliseconds,
                      segment.endsAtMilliseconds,
                    ),
                  )}
                  onChange={(event: ChangeEvent<HTMLTextAreaElement>) =>
                    changeDraft((current) =>
                      updateMeetingTranscriptSegment(current, segment.localId, {
                        text: event.target.value,
                      }),
                    )
                  }
                />
                <div className="meeting-transcript-editor__segment-actions">
                  <button
                    type="button"
                    disabled={
                      busy ||
                      !canMergeMeetingTranscriptSegment(
                        draftState.present,
                        segment.localId,
                        "previous",
                      )
                    }
                    onClick={() =>
                      changeDraft((current) =>
                        mergeMeetingTranscriptSegment(
                          current,
                          segment.localId,
                          "previous",
                        ),
                      )
                    }
                  >
                    <CornerUpLeft aria-hidden="true" />
                    <span aria-hidden="true">
                      {copy.meetings.mergePrevious}
                    </span>
                    <span className="sr-only">
                      {copy.meetings.mergePreviousAt(segment.ordinal + 1)}
                    </span>
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => {
                      const cursor =
                        textareaRefs.current.get(segment.localId)
                          ?.selectionStart ?? 0;
                      changeDraft((current) =>
                        splitMeetingTranscriptSegment(
                          current,
                          segment.localId,
                          cursor,
                          createUuidV7(),
                        ),
                      );
                    }}
                  >
                    <Scissors aria-hidden="true" />
                    <span aria-hidden="true">
                      {copy.meetings.splitAtCursor}
                    </span>
                    <span className="sr-only">
                      {copy.meetings.splitAtCursorAt(segment.ordinal + 1)}
                    </span>
                  </button>
                  <button
                    type="button"
                    disabled={
                      busy ||
                      !canMergeMeetingTranscriptSegment(
                        draftState.present,
                        segment.localId,
                        "next",
                      )
                    }
                    onClick={() =>
                      changeDraft((current) =>
                        mergeMeetingTranscriptSegment(
                          current,
                          segment.localId,
                          "next",
                        ),
                      )
                    }
                  >
                    <CornerDownRight aria-hidden="true" />
                    <span aria-hidden="true">{copy.meetings.mergeNext}</span>
                    <span className="sr-only">
                      {copy.meetings.mergeNextAt(segment.ordinal + 1)}
                    </span>
                  </button>
                </div>
              </li>
            ))}
          </ol>
        </section>
      </div>

      <footer className="meeting-transcript-editor__footer">
        <div
          className="meeting-transcript-editor__save-state"
          data-state={saveState}
          role={
            saveState === "failed" ||
            saveState === "invalid" ||
            saveState === "conflict" ||
            saveState === "reanalyze_failed"
              ? "alert"
              : "status"
          }
        >
          {saveState === "failed" ||
          saveState === "invalid" ||
          saveState === "conflict" ||
          saveState === "reanalyze_failed" ? (
            <CircleAlert aria-hidden="true" />
          ) : saveState === "saving" || saveState === "reloading" ? (
            <LoaderCircle className="spin" aria-hidden="true" />
          ) : (
            <Save aria-hidden="true" />
          )}
          <span>{saveStateCopy(saveState)}</span>
          {saveState === "conflict" && (
            <button
              className="text-button focus-visible-control"
              type="button"
              onClick={() => void reloadLatest()}
            >
              {copy.meetings.reloadTranscript}
            </button>
          )}
        </div>
        <div className="meeting-transcript-editor__footer-actions">
          <button
            className="text-button focus-visible-control"
            type="button"
            disabled={draftState.past.length === 0 || busy}
            onClick={undo}
          >
            <Undo2 aria-hidden="true" />
            {copy.meetings.undoTranscriptEdit}
          </button>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={!dirty || !valid || busy}
            onClick={() => void saveNow()}
          >
            {saveState === "saving" && (
              <LoaderCircle className="spin" aria-hidden="true" />
            )}
            {copy.meetings.saveTranscriptNow}
          </button>
          <button
            className="primary-button focus-visible-control"
            type="button"
            disabled={!valid || busy}
            onClick={() => void reanalyze()}
          >
            {reanalyzing ? (
              <LoaderCircle className="spin" aria-hidden="true" />
            ) : (
              <Sparkles aria-hidden="true" />
            )}
            {reanalyzing
              ? copy.meetings.reanalyzingTranscript
              : copy.meetings.reanalyzeTranscript}
          </button>
        </div>
      </footer>
    </section>
  );
}

function nextUnassignedParticipant(
  participants: string[],
  draft: MeetingTranscriptDraft,
): string {
  const assignedNames = new Set(
    draft.speakers
      .map((speaker) => speaker.displayName.trim().toLocaleLowerCase("ko-KR"))
      .filter((name): name is string => Boolean(name)),
  );
  return (
    participants
      .find((participant) => {
        const name = participant.trim();
        return name && !assignedNames.has(name.toLocaleLowerCase("ko-KR"));
      })
      ?.trim() ?? ""
  );
}

function saveStateCopy(state: TranscriptSaveState): string {
  switch (state) {
    case "pending":
      return copy.meetings.transcriptSavePending;
    case "saving":
      return copy.meetings.transcriptSaving;
    case "reloading":
      return copy.meetings.transcriptReloading;
    case "saved":
      return copy.meetings.transcriptSaved;
    case "invalid":
      return copy.meetings.transcriptInvalid;
    case "conflict":
      return copy.meetings.transcriptConflict;
    case "failed":
      return copy.meetings.transcriptSaveRetryCopy;
    case "reanalyze_failed":
      return copy.meetings.transcriptReanalyzeRetryCopy;
    default:
      return copy.meetings.transcriptAutosaveReady;
  }
}

function segmentRange(start: number, end: number): string {
  return `${segmentTimestamp(start)}–${segmentTimestamp(end)}`;
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
