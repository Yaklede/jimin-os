import {
  CalendarPlus,
  Check,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Clock3,
  FileAudio,
  FolderKanban,
  ListChecks,
  LoaderCircle,
  Mic,
  Pencil,
  Plus,
  Quote,
  Save,
  Trash2,
  Users,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import {
  cancelMeetingRecording,
  createMeeting,
  deleteMeeting,
  decideMeetingAction,
  finalizeMeetingRecording,
  fetchMeeting,
  fetchMeetings,
  reanalyzeMeeting,
  startMeetingRecording,
  updateMeetingTranscript,
  updateMeetingAction,
  updateMeetingRecordingNotes,
  uploadMeetingRecordingChunk,
  type Meeting,
  type MeetingActionItem,
  type MeetingDetail,
  type MeetingSummary,
  type MeetingTranscriptUpdateInput,
  type MeetingTranscriptUpdateResult,
} from "../api/meetings";
import { type Project, type Workspace } from "../api/projects";
import { copy } from "../copy";
import { registerMobileBackHandler } from "../mobileBack";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";
import { MeetingTranscriptPanel } from "./MeetingTranscriptPanel";
import { applyMeetingTranscriptUpdateToDetail } from "./meetingTranscriptDraft";

export {
  groupTranscriptSegments,
  type MeetingTranscriptGroup,
} from "./MeetingTranscriptPanel";

const RECORDER_METER_WEIGHTS = [
  0.42, 0.68, 0.9, 0.58, 1, 0.76, 0.48, 0.82, 0.62, 0.94, 0.7, 0.46,
];

type MeetingsWorkspaceProps = {
  apiBaseUrl: string;
  accessToken: string;
  workspaces: Workspace[];
  projects: Project[];
  selectedWorkspaceId: string | undefined;
  onSelectWorkspace(workspaceId: string): void;
};

export function MeetingsWorkspace({
  apiBaseUrl,
  accessToken,
  workspaces,
  projects,
  selectedWorkspaceId,
  onSelectWorkspace,
}: MeetingsWorkspaceProps) {
  const [meetings, setMeetings] = useState<MeetingSummary[]>([]);
  const [selectedMeetingId, setSelectedMeetingId] = useState<string>();
  const [detail, setDetail] = useState<MeetingDetail>();
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [creating, setCreating] = useState(false);
  const [showComposer, setShowComposer] = useState(false);
  const [error, setError] = useState<string>();
  const [decisionBusyId, setDecisionBusyId] = useState<string>();
  const [savingItemId, setSavingItemId] = useState<string>();
  const [bulkApplying, setBulkApplying] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [success, setSuccess] = useState<string>();
  const [mobileListOpen, setMobileListOpen] = useState(false);
  const [transcriptDirty, setTranscriptDirty] = useState(false);
  const meetingListRef = useRef<HTMLDivElement>(null);
  const successTimerRef = useRef<number | undefined>(undefined);
  const skeletonVisible = useDelayedSkeleton(loading || detailLoading);

  const loadList = useCallback(async () => {
    try {
      const items = await fetchMeetings(apiBaseUrl, accessToken);
      setMeetings(items);
      setSelectedMeetingId((current) => current ?? items[0]?.id);
      setError(undefined);
    } catch {
      setError(copy.meetings.loadFailed);
    } finally {
      setLoading(false);
    }
  }, [accessToken, apiBaseUrl]);

  const loadDetail = useCallback(
    async (meetingId: string, quiet = false) => {
      if (!quiet) setDetailLoading(true);
      try {
        const next = await fetchMeeting(apiBaseUrl, accessToken, meetingId);
        setDetail(next);
        setMeetings((current) =>
          current.map((meeting) =>
            meeting.id === next.id ? { ...meeting, ...next } : meeting,
          ),
        );
        setError(undefined);
      } catch {
        if (!quiet) setError(copy.meetings.detailFailed);
      } finally {
        if (!quiet) setDetailLoading(false);
      }
    },
    [accessToken, apiBaseUrl],
  );

  useEffect(() => {
    void loadList();
  }, [loadList]);

  useEffect(
    () => () => {
      if (successTimerRef.current) window.clearTimeout(successTimerRef.current);
    },
    [],
  );

  useEffect(() => {
    if (!selectedMeetingId) {
      setDetail(undefined);
      return;
    }
    void loadDetail(selectedMeetingId);
  }, [loadDetail, selectedMeetingId]);

  useEffect(() => {
    if (
      !detail ||
      !["recording", "transcribing", "queued", "analyzing"].includes(
        detail.status,
      )
    )
      return;
    const timer = window.setInterval(() => {
      void loadDetail(detail.id, true);
    }, 1_800);
    return () => window.clearInterval(timer);
  }, [detail, loadDetail]);

  useEffect(() => {
    if (!window.matchMedia("(max-width: 720px)").matches) return;
    const list = meetingListRef.current;
    const active = list?.querySelector<HTMLElement>('[data-active="true"]');
    if (!list || !active) return;

    const reduceMotion = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    list.scrollTo({
      left: Math.max(0, active.offsetLeft - 16),
      behavior: reduceMotion ? "auto" : "smooth",
    });
  }, [meetings.length, selectedMeetingId]);

  async function submitMeeting(input: MeetingComposerInput) {
    setCreating(true);
    setError(undefined);
    try {
      const created = await createMeeting(apiBaseUrl, accessToken, input);
      setMeetings((current) => [created, ...current]);
      setSelectedMeetingId(created.id);
      setShowComposer(false);
    } catch {
      setError(copy.meetings.createFailed);
    } finally {
      setCreating(false);
    }
  }

  async function recordedMeeting(meetingId: string) {
    setShowComposer(false);
    await loadList();
    setSelectedMeetingId(meetingId);
  }

  async function decide(
    item: MeetingActionItem,
    decision: "approve" | "reject",
  ) {
    if (!detail) return;
    setDecisionBusyId(item.id);
    setError(undefined);
    try {
      await decideMeetingAction(
        apiBaseUrl,
        accessToken,
        detail.id,
        item.id,
        decision,
      );
      await loadDetail(detail.id, true);
    } catch {
      setError(
        decision === "approve"
          ? copy.meetings.applyFailed
          : copy.meetings.rejectFailed,
      );
    } finally {
      setDecisionBusyId(undefined);
    }
  }

  async function retryAnalysis() {
    if (!detail) return;
    setRetrying(true);
    setError(undefined);
    try {
      const queued = await reanalyzeMeeting(
        apiBaseUrl,
        accessToken,
        detail.id,
        detail.version,
      );
      setDetail((current) =>
        current?.id === queued.id ? { ...current, ...queued } : current,
      );
      setMeetings((current) =>
        current.map((meeting) =>
          meeting.id === queued.id ? { ...meeting, ...queued } : meeting,
        ),
      );
      setTranscriptDirty(false);
      void loadDetail(detail.id, true);
    } catch {
      setError(copy.meetings.retryFailed);
    } finally {
      setRetrying(false);
    }
  }

  async function saveTranscript(
    input: MeetingTranscriptUpdateInput,
  ): Promise<MeetingTranscriptUpdateResult> {
    if (!detail) throw new Error("meeting_not_selected");
    const saved = await updateMeetingTranscript(
      apiBaseUrl,
      accessToken,
      detail.id,
      input,
    );
    const savedDetail = applyMeetingTranscriptUpdateToDetail(
      detail,
      input,
      saved.version,
    );
    setDetail(savedDetail);
    setMeetings((current) =>
      current.map((meeting) =>
        meeting.id === detail.id
          ? {
              ...meeting,
              version: saved.version,
              analyzedAt: null,
              updatedAt: savedDetail.updatedAt,
            }
          : meeting,
      ),
    );
    return saved;
  }

  async function reloadTranscript(): Promise<MeetingDetail> {
    if (!detail) throw new Error("meeting_not_selected");
    const latest = await fetchMeeting(apiBaseUrl, accessToken, detail.id);
    setDetail(latest);
    setMeetings((current) =>
      current.map((meeting) =>
        meeting.id === latest.id ? { ...meeting, ...latest } : meeting,
      ),
    );
    return latest;
  }

  async function reanalyzeTranscript(expectedVersion: number): Promise<void> {
    if (!detail) return;
    setRetrying(true);
    try {
      const queued = await reanalyzeMeeting(
        apiBaseUrl,
        accessToken,
        detail.id,
        expectedVersion,
      );
      setDetail((current) =>
        current?.id === queued.id ? { ...current, ...queued } : current,
      );
      setMeetings((current) =>
        current.map((meeting) =>
          meeting.id === queued.id ? { ...meeting, ...queued } : meeting,
        ),
      );
      setTranscriptDirty(false);
      void loadDetail(detail.id, true);
    } finally {
      setRetrying(false);
    }
  }

  function selectMeeting(meetingId: string) {
    if (meetingId === selectedMeetingId) {
      setMobileListOpen(false);
      return;
    }
    if (
      transcriptDirty &&
      !window.confirm(copy.meetings.transcriptDiscardConfirm)
    ) {
      return;
    }
    setTranscriptDirty(false);
    setSelectedMeetingId(meetingId);
    setMobileListOpen(false);
  }

  async function updateAction(
    item: MeetingActionItem,
    input: Parameters<typeof updateMeetingAction>[4],
  ) {
    if (!detail) return false;
    setSavingItemId(item.id);
    setError(undefined);
    try {
      await updateMeetingAction(
        apiBaseUrl,
        accessToken,
        detail.id,
        item,
        input,
      );
      await loadDetail(detail.id, true);
      return true;
    } catch {
      setError(copy.meetings.updateFailed);
      return false;
    } finally {
      setSavingItemId(undefined);
    }
  }

  async function applyRemaining() {
    if (!detail) return;
    const pending = detail.actionItems.filter(
      (item) => item.status === "suggested",
    );
    if (pending.length === 0) return;
    setBulkApplying(true);
    setError(undefined);
    try {
      for (const item of pending) {
        await decideMeetingAction(
          apiBaseUrl,
          accessToken,
          detail.id,
          item.id,
          "approve",
        );
      }
      await loadDetail(detail.id, true);
    } catch {
      setError(copy.meetings.bulkApplyFailed);
      await loadDetail(detail.id, true);
    } finally {
      setBulkApplying(false);
    }
  }

  async function removeSelectedMeeting(): Promise<boolean> {
    if (!detail) return false;
    setDeleting(true);
    setError(undefined);
    setSuccess(undefined);
    try {
      await deleteMeeting(apiBaseUrl, accessToken, detail.id, detail.version);
      const deletedIndex = meetings.findIndex(
        (meeting) => meeting.id === detail.id,
      );
      const remaining = meetings.filter((meeting) => meeting.id !== detail.id);
      const next =
        remaining[
          Math.min(Math.max(0, deletedIndex), Math.max(0, remaining.length - 1))
        ];
      setMeetings(remaining);
      setDetail(undefined);
      setSelectedMeetingId(next?.id);
      setTranscriptDirty(false);
      setSuccess(copy.meetings.deleteSuccess);
      if (successTimerRef.current) {
        window.clearTimeout(successTimerRef.current);
      }
      successTimerRef.current = window.setTimeout(
        () => setSuccess(undefined),
        5_000,
      );
      return true;
    } catch {
      await loadDetail(detail.id, true);
      setError(copy.meetings.deleteErrorRetry);
      return false;
    } finally {
      setDeleting(false);
    }
  }

  return (
    <section className="meetings-page" aria-labelledby="meetings-title">
      <header className="page-heading meetings-page__heading">
        <div>
          <span>{copy.meetings.eyebrow}</span>
          <h1 id="meetings-title">{copy.meetings.title}</h1>
          <p>{copy.meetings.description}</p>
        </div>
        <button
          className="primary-button focus-visible-control"
          type="button"
          onClick={() => setShowComposer(true)}
        >
          <Plus aria-hidden="true" />
          {copy.meetings.newMeeting}
        </button>
      </header>

      {error && (
        <div className="workspace-notice" role="alert">
          <CircleAlert aria-hidden="true" />
          <span>{error}</span>
          <button type="button" onClick={() => void loadList()}>
            {copy.actions.checkAgain}
          </button>
        </div>
      )}
      {success && (
        <div className="workspace-notice" data-tone="success" role="status">
          <Check aria-hidden="true" />
          <span>{success}</span>
        </div>
      )}

      <div className="meetings-layout">
        <aside
          className="meetings-list"
          aria-label={copy.meetings.listLabel}
          data-mobile-expanded={mobileListOpen}
        >
          <div className="meetings-section-heading">
            <h2>{copy.meetings.recent}</h2>
            <span>{copy.meetings.count(meetings.length)}</span>
            <button
              className="meetings-list__mobile-toggle focus-visible-control"
              type="button"
              aria-expanded={mobileListOpen}
              onClick={() => setMobileListOpen((current) => !current)}
            >
              {mobileListOpen
                ? copy.meetings.collapseList
                : copy.meetings.openList}
              <ChevronDown aria-hidden="true" />
            </button>
          </div>
          {loading && meetings.length === 0 && (
            <SkeletonGroup
              className="meetings-list__skeleton"
              label={copy.meetings.loading}
              visible={skeletonVisible}
            >
              <SkeletonBlock />
              <SkeletonBlock />
              <SkeletonBlock />
            </SkeletonGroup>
          )}
          {!loading && meetings.length === 0 ? (
            <EmptyMeetings onCreate={() => setShowComposer(true)} />
          ) : (
            <div className="meetings-list__items" ref={meetingListRef}>
              {meetings.map((meeting) => (
                <button
                  className="meeting-list-item focus-visible-control"
                  data-active={meeting.id === selectedMeetingId}
                  type="button"
                  key={meeting.id}
                  onClick={() => selectMeeting(meeting.id)}
                >
                  <span className="meeting-list-item__icon" aria-hidden="true">
                    <FileAudio />
                  </span>
                  <span className="meeting-list-item__content">
                    <strong>{meeting.title}</strong>
                    <small>
                      {meeting.projectTitle ?? copy.meetings.noProject} ·{" "}
                      {shortDate(meeting.createdAt)}
                    </small>
                    <MeetingStatusLabel status={meeting.status} />
                  </span>
                  <ChevronRight aria-hidden="true" />
                </button>
              ))}
            </div>
          )}
        </aside>

        <main className="meeting-detail" aria-live="polite">
          {detailLoading && !detail ? (
            <MeetingDetailSkeleton visible={skeletonVisible} />
          ) : detail ? (
            <MeetingReview
              key={detail.id}
              detail={detail}
              busyItemId={decisionBusyId}
              savingItemId={savingItemId}
              bulkApplying={bulkApplying}
              retrying={retrying}
              deleting={deleting}
              onDecide={decide}
              onUpdate={updateAction}
              onApplyRemaining={applyRemaining}
              onRetry={retryAnalysis}
              onSaveTranscript={saveTranscript}
              onReloadTranscript={reloadTranscript}
              onReanalyzeTranscript={reanalyzeTranscript}
              onTranscriptDirtyChange={setTranscriptDirty}
              onDelete={removeSelectedMeeting}
            />
          ) : (
            <div className="meeting-detail__empty">
              <FileAudio aria-hidden="true" />
              <h2>{copy.meetings.selectTitle}</h2>
              <p>{copy.meetings.selectDescription}</p>
            </div>
          )}
        </main>
      </div>

      {showComposer && (
        <MeetingComposer
          apiBaseUrl={apiBaseUrl}
          accessToken={accessToken}
          workspaces={workspaces}
          projects={projects}
          selectedWorkspaceId={selectedWorkspaceId}
          saving={creating}
          onSelectWorkspace={onSelectWorkspace}
          onClose={() => setShowComposer(false)}
          onRecorded={recordedMeeting}
          onSubmit={submitMeeting}
        />
      )}
    </section>
  );
}

type MeetingComposerInput = Parameters<typeof createMeeting>[2];

function MeetingComposer({
  apiBaseUrl,
  accessToken,
  workspaces,
  projects,
  selectedWorkspaceId,
  saving,
  onSelectWorkspace,
  onClose,
  onRecorded,
  onSubmit,
}: {
  apiBaseUrl: string;
  accessToken: string;
  workspaces: Workspace[];
  projects: Project[];
  selectedWorkspaceId: string | undefined;
  saving: boolean;
  onSelectWorkspace(workspaceId: string): void;
  onClose(): void;
  onRecorded(meetingId: string): Promise<void>;
  onSubmit(input: MeetingComposerInput): Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [purpose, setPurpose] = useState("");
  const [participants, setParticipants] = useState("");
  const [projectId, setProjectId] = useState("");
  const [transcript, setTranscript] = useState("");
  const [notes, setNotes] = useState("");
  const [recordingSession, setRecordingSession] = useState<{
    meetingId: string;
    recordingId: string;
    mimeType: string;
    startedAt: number;
  }>();
  const [recordingSeconds, setRecordingSeconds] = useState(0);
  const [recordingBusy, setRecordingBusy] = useState(false);
  const [recordingError, setRecordingError] = useState<string>();
  const [exitConfirmOpen, setExitConfirmOpen] = useState(false);
  const [notesState, setNotesState] = useState<
    "idle" | "saving" | "saved" | "failed"
  >("idle");
  const recorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const sequenceRef = useRef(0);
  const uploadChainRef = useRef<Promise<void>>(Promise.resolve());
  const notesRef = useRef(notes);
  const pendingWindowCloseRef = useRef(false);
  const audioContextRef = useRef<AudioContext | null>(null);
  const audioMeterFrameRef = useRef<number | null>(null);
  const audioMeterLastUpdateRef = useRef(0);
  const [audioLevel, setAudioLevel] = useState(0);

  useEffect(() => {
    notesRef.current = notes;
  }, [notes]);

  const recordingActive = Boolean(recordingSession);

  const requestClose = useCallback(() => {
    if (recordingActive) {
      setExitConfirmOpen(true);
      return;
    }
    if (!saving && !recordingBusy) onClose();
  }, [onClose, recordingActive, recordingBusy, saving]);

  useEffect(() => {
    return registerMobileBackHandler(() => {
      requestClose();
      return true;
    }, 110);
  }, [requestClose]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      if (exitConfirmOpen) {
        setExitConfirmOpen(false);
        return;
      }
      requestClose();
    }
    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, [exitConfirmOpen, requestClose]);

  useEffect(() => {
    if (!recordingActive) return;
    function beforeUnload(event: BeforeUnloadEvent) {
      event.preventDefault();
      event.returnValue = "";
    }
    window.addEventListener("beforeunload", beforeUnload);
    return () => window.removeEventListener("beforeunload", beforeUnload);
  }, [recordingActive]);

  useEffect(() => {
    if (!recordingActive || !("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) =>
        getCurrentWindow().onCloseRequested((event) => {
          event.preventDefault();
          pendingWindowCloseRef.current = true;
          setExitConfirmOpen(true);
        }),
      )
      .then((nextUnlisten) => {
        if (disposed) {
          nextUnlisten();
          return;
        }
        unlisten = nextUnlisten;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [recordingActive]);

  useEffect(() => {
    if (!recordingSession) return;
    const timer = window.setInterval(() => {
      setRecordingSeconds(
        Math.max(
          0,
          Math.floor((Date.now() - recordingSession.startedAt) / 1_000),
        ),
      );
    }, 1_000);
    return () => window.clearInterval(timer);
  }, [recordingSession]);

  useEffect(() => {
    if (!recordingSession) return;
    setNotesState("saving");
    const timer = window.setTimeout(() => {
      void updateMeetingRecordingNotes(
        apiBaseUrl,
        accessToken,
        recordingSession.recordingId,
        notes,
      )
        .then(() => setNotesState("saved"))
        .catch(() => setNotesState("failed"));
    }, 700);
    return () => window.clearTimeout(timer);
  }, [accessToken, apiBaseUrl, notes, recordingSession]);

  const stopAudioMeter = useCallback((resetLevel = true) => {
    if (audioMeterFrameRef.current !== null) {
      window.cancelAnimationFrame(audioMeterFrameRef.current);
      audioMeterFrameRef.current = null;
    }
    const context = audioContextRef.current;
    audioContextRef.current = null;
    if (context && context.state !== "closed") {
      void context.close();
    }
    if (resetLevel) setAudioLevel(0);
  }, []);

  const startAudioMeter = useCallback(
    (stream: MediaStream) => {
      stopAudioMeter();
      if (typeof window.AudioContext === "undefined") return;

      const context = new window.AudioContext();
      const analyser = context.createAnalyser();
      analyser.fftSize = 256;
      analyser.smoothingTimeConstant = 0.72;
      context.createMediaStreamSource(stream).connect(analyser);
      audioContextRef.current = context;
      audioMeterLastUpdateRef.current = 0;
      const samples = new Uint8Array(analyser.fftSize);

      const measure = (timestamp: number) => {
        analyser.getByteTimeDomainData(samples);
        if (timestamp - audioMeterLastUpdateRef.current >= 80) {
          let squared = 0;
          for (const sample of samples) {
            const normalized = (sample - 128) / 128;
            squared += normalized * normalized;
          }
          const level = Math.min(1, Math.sqrt(squared / samples.length) * 3.8);
          setAudioLevel(level);
          audioMeterLastUpdateRef.current = timestamp;
        }
        audioMeterFrameRef.current = window.requestAnimationFrame(measure);
      };

      void context.resume().catch(() => undefined);
      audioMeterFrameRef.current = window.requestAnimationFrame(measure);
    },
    [stopAudioMeter],
  );

  useEffect(
    () => () => {
      recorderRef.current?.stop();
      streamRef.current?.getTracks().forEach((track) => track.stop());
      stopAudioMeter(false);
    },
    [stopAudioMeter],
  );

  function enqueueChunk(
    blob: Blob,
    session: NonNullable<typeof recordingSession>,
  ) {
    if (blob.size === 0) return;
    const sequence = sequenceRef.current;
    sequenceRef.current += 1;
    uploadChainRef.current = uploadChainRef.current.then(async () => {
      await uploadMeetingRecordingChunk(
        apiBaseUrl,
        accessToken,
        session.recordingId,
        sequence,
        blob,
        session.mimeType,
      );
    });
    uploadChainRef.current.catch(() =>
      setRecordingError(copy.meetings.recordingUploadFailed),
    );
  }

  async function startRecording() {
    if (!title.trim() || recordingBusy) {
      setRecordingError(copy.meetings.recordingNameRequired);
      return;
    }
    if (
      !navigator.mediaDevices?.getUserMedia ||
      typeof MediaRecorder === "undefined"
    ) {
      setRecordingError(copy.meetings.recordingUnsupported);
      return;
    }
    setRecordingBusy(true);
    setRecordingError(undefined);
    let created: Awaited<ReturnType<typeof startMeetingRecording>> | undefined;
    try {
      created = await startMeetingRecording(apiBaseUrl, accessToken, {
        title: title.trim(),
        purpose: purpose.trim() || undefined,
        participants: normalizedParticipants(participants),
        workspaceId: selectedWorkspaceId,
        projectId: projectId || undefined,
        startedAt: new Date().toISOString(),
      });
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: meetingRecordingAudioConstraints(),
      });
      streamRef.current = stream;
      const audioTrack = stream.getAudioTracks()[0];
      audioTrack?.addEventListener(
        "ended",
        () => setRecordingError(copy.meetings.recordingInterrupted),
        { once: true },
      );
      startAudioMeter(stream);
      const mimeType = preferredRecordingMimeType();
      const recorder = mimeType
        ? new MediaRecorder(stream, { mimeType })
        : new MediaRecorder(stream);
      const session = {
        meetingId: created.meeting.id,
        recordingId: created.recording.id,
        mimeType: recorder.mimeType || mimeType || "audio/webm",
        startedAt: Date.now(),
      };
      sequenceRef.current = 0;
      uploadChainRef.current = Promise.resolve();
      recorderRef.current = recorder;
      recorder.addEventListener("dataavailable", (event) => {
        enqueueChunk(event.data, session);
      });
      recorder.start(4_000);
      setRecordingSession(session);
      setRecordingSeconds(0);
      setNotes("");
      setNotesState("idle");
    } catch {
      streamRef.current?.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
      stopAudioMeter();
      if (created) {
        await cancelMeetingRecording(
          apiBaseUrl,
          accessToken,
          created.recording.id,
        ).catch(() => undefined);
      }
      setRecordingError(copy.meetings.recordingPermission);
    } finally {
      setRecordingBusy(false);
    }
  }

  async function stopMediaRecorder() {
    const recorder = recorderRef.current;
    if (!recorder || recorder.state === "inactive") return;
    await new Promise<void>((resolve) => {
      recorder.addEventListener("stop", () => resolve(), { once: true });
      recorder.stop();
    });
    streamRef.current?.getTracks().forEach((track) => track.stop());
    recorderRef.current = null;
    streamRef.current = null;
    stopAudioMeter();
  }

  async function destroyWindowIfRequested() {
    if (!pendingWindowCloseRef.current || !("__TAURI_INTERNALS__" in window))
      return;
    pendingWindowCloseRef.current = false;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().destroy();
  }

  async function saveRecordingAndClose() {
    if (!recordingSession || recordingBusy) return;
    setRecordingBusy(true);
    setRecordingError(undefined);
    try {
      await updateMeetingRecordingNotes(
        apiBaseUrl,
        accessToken,
        recordingSession.recordingId,
        notesRef.current,
      );
      await stopMediaRecorder();
      await uploadChainRef.current;
      await finalizeMeetingRecording(
        apiBaseUrl,
        accessToken,
        recordingSession.recordingId,
        {
          mimeType: recordingSession.mimeType,
          durationMilliseconds: Math.max(
            1,
            Date.now() - recordingSession.startedAt,
          ),
        },
      );
      const meetingId = recordingSession.meetingId;
      setRecordingSession(undefined);
      setExitConfirmOpen(false);
      await onRecorded(meetingId);
      await destroyWindowIfRequested();
    } catch {
      setRecordingError(copy.meetings.recordingFinishFailed);
    } finally {
      setRecordingBusy(false);
    }
  }

  async function discardRecording() {
    if (!recordingSession || recordingBusy) return;
    setRecordingBusy(true);
    setRecordingError(undefined);
    try {
      await stopMediaRecorder();
      await uploadChainRef.current.catch(() => undefined);
      await cancelMeetingRecording(
        apiBaseUrl,
        accessToken,
        recordingSession.recordingId,
      );
      setRecordingSession(undefined);
      setExitConfirmOpen(false);
      onClose();
      await destroyWindowIfRequested();
    } catch {
      setRecordingError(copy.meetings.recordingDiscardFailed);
    } finally {
      setRecordingBusy(false);
    }
  }

  async function submit() {
    await onSubmit({
      title: title.trim(),
      purpose: purpose.trim() || undefined,
      participants: normalizedParticipants(participants),
      transcript: transcript.trim(),
      workspaceId: selectedWorkspaceId,
      projectId: projectId || undefined,
    });
  }

  const canSubmit = title.trim().length > 0 && transcript.trim().length > 0;

  return createPortal(
    <div
      className="modal-backdrop"
      role="presentation"
      onMouseDown={requestClose}
    >
      <section
        className="meeting-composer"
        data-recording={recordingActive}
        role="dialog"
        aria-modal="true"
        aria-labelledby="meeting-composer-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <span>{copy.meetings.composerEyebrow}</span>
            <h2 id="meeting-composer-title">
              {recordingActive
                ? copy.meetings.recordingTitle
                : copy.meetings.composerTitle}
            </h2>
            <p>
              {recordingActive
                ? copy.meetings.recordingSignalDescription
                : copy.meetings.composerDescription}
            </p>
          </div>
          <button
            className="icon-button focus-visible-control"
            type="button"
            aria-label={
              recordingActive
                ? copy.meetings.openRecordingExit
                : copy.actions.cancel
            }
            onClick={requestClose}
          >
            <X aria-hidden="true" />
          </button>
        </header>

        {!recordingActive ? (
          <div className="meeting-composer__fields">
            <label>
              <span>{copy.meetings.nameLabel}</span>
              <input
                value={title}
                maxLength={200}
                placeholder={copy.meetings.namePlaceholder}
                onChange={(event) => setTitle(event.target.value)}
              />
            </label>
            <label>
              <span>{copy.meetings.purposeLabel}</span>
              <input
                value={purpose}
                maxLength={2_000}
                placeholder={copy.meetings.purposePlaceholder}
                onChange={(event) => setPurpose(event.target.value)}
              />
            </label>
            <label>
              <span>{copy.meetings.participantsLabel}</span>
              <input
                value={participants}
                placeholder={copy.meetings.participantsPlaceholder}
                onChange={(event) => setParticipants(event.target.value)}
              />
            </label>
            <div className="meeting-composer__scope">
              <label>
                <span>{copy.meetings.workspaceLabel}</span>
                <select
                  value={selectedWorkspaceId ?? ""}
                  onChange={(event) => {
                    setProjectId("");
                    onSelectWorkspace(event.target.value);
                  }}
                >
                  {workspaces.map((workspace) => (
                    <option key={workspace.id} value={workspace.id}>
                      {workspace.name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                <span>{copy.meetings.projectLabel}</span>
                <select
                  value={projectId}
                  onChange={(event) => setProjectId(event.target.value)}
                >
                  <option value="">{copy.meetings.noProject}</option>
                  {projects.map((project) => (
                    <option key={project.id} value={project.id}>
                      {project.title}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <label className="meeting-composer__transcript">
              <span>{copy.meetings.transcriptLabel}</span>
              <textarea
                value={transcript}
                maxLength={120_000}
                rows={7}
                placeholder={copy.meetings.transcriptPlaceholder}
                onChange={(event) => setTranscript(event.target.value)}
              />
            </label>
          </div>
        ) : (
          <div className="meeting-recorder">
            <span className="sr-only" role="status">
              {copy.meetings.recordingTitle}
            </span>
            <MeetingPipeline stage="recording" />
            <div className="meeting-recorder__status">
              <span className="meeting-recorder__pulse" aria-hidden="true" />
              <div>
                <strong>
                  {audioLevel > 0.035
                    ? copy.meetings.recordingSignalActive
                    : copy.meetings.recordingSignalWaiting}
                </strong>
                <p>{copy.meetings.recordingDescription}</p>
              </div>
              <time
                aria-label={copy.meetings.recordingElapsed(
                  recordingTime(recordingSeconds),
                )}
              >
                {recordingTime(recordingSeconds)}
              </time>
              <div className="meeting-recorder__meter" aria-hidden="true">
                {RECORDER_METER_WEIGHTS.map((weight, index) => (
                  <span
                    key={`${weight}-${index}`}
                    style={{
                      height: `${Math.max(
                        6,
                        Math.round(7 + audioLevel * weight * 38),
                      )}px`,
                    }}
                  />
                ))}
              </div>
            </div>
            <label className="meeting-recorder__notes">
              <span>
                <strong>{copy.meetings.notesPadLabel}</strong>
                <small data-state={notesState}>
                  {notesSaveLabel(notesState)}
                </small>
              </span>
              <textarea
                autoFocus
                value={notes}
                maxLength={40_000}
                rows={12}
                placeholder={copy.meetings.notesPadPlaceholder}
                onChange={(event) => setNotes(event.target.value)}
              />
            </label>
          </div>
        )}

        {!recordingActive && (
          <div className="meeting-composer__dictation" data-active="false">
            <button
              className="dictation-button focus-visible-control"
              type="button"
              disabled={!title.trim() || recordingBusy}
              onClick={() => void startRecording()}
            >
              {recordingBusy ? (
                <LoaderCircle className="spin" aria-hidden="true" />
              ) : (
                <Mic aria-hidden="true" />
              )}
              {recordingBusy
                ? copy.meetings.startingRecording
                : copy.meetings.startRecording}
            </button>
            <div>
              <strong>{copy.meetings.recordingReadyTitle}</strong>
              <p>{copy.meetings.recordingReadyDescription}</p>
            </div>
          </div>
        )}
        {recordingError && (
          <p className="meeting-composer__error" role="alert">
            {recordingError}
          </p>
        )}

        <footer>
          {recordingActive ? (
            <>
              <button
                className="secondary-button"
                type="button"
                disabled={recordingBusy}
                onClick={requestClose}
              >
                {copy.meetings.closeRecording}
              </button>
              <button
                className="primary-button"
                type="button"
                disabled={recordingBusy}
                onClick={() => void saveRecordingAndClose()}
              >
                {recordingBusy && (
                  <LoaderCircle className="spin" aria-hidden="true" />
                )}
                {recordingBusy
                  ? copy.meetings.savingRecording
                  : copy.meetings.finishRecording}
              </button>
            </>
          ) : (
            <>
              <button
                className="secondary-button"
                type="button"
                onClick={requestClose}
              >
                {copy.actions.cancel}
              </button>
              <button
                className="primary-button"
                type="button"
                disabled={!canSubmit || saving}
                onClick={() => void submit()}
              >
                {saving && <LoaderCircle className="spin" aria-hidden="true" />}
                {saving ? copy.meetings.queuing : copy.meetings.analyze}
              </button>
            </>
          )}
        </footer>

        {exitConfirmOpen && (
          <div
            className="meeting-recording-exit"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="meeting-recording-exit-title"
          >
            <div>
              <CircleAlert aria-hidden="true" />
              <h3 id="meeting-recording-exit-title">
                {copy.meetings.exitRecordingTitle}
              </h3>
              <p>{copy.meetings.exitRecordingDescription}</p>
              <div>
                <button
                  className="secondary-button"
                  type="button"
                  disabled={recordingBusy}
                  onClick={() => {
                    pendingWindowCloseRef.current = false;
                    setExitConfirmOpen(false);
                  }}
                >
                  {copy.meetings.continueRecording}
                </button>
                <button
                  className="secondary-button meeting-recording-exit__discard"
                  type="button"
                  disabled={recordingBusy}
                  onClick={() => void discardRecording()}
                >
                  {copy.meetings.discardRecording}
                </button>
                <button
                  className="primary-button"
                  type="button"
                  disabled={recordingBusy}
                  onClick={() => void saveRecordingAndClose()}
                >
                  {recordingBusy && (
                    <LoaderCircle className="spin" aria-hidden="true" />
                  )}
                  {copy.meetings.saveAndExitRecording}
                </button>
              </div>
            </div>
          </div>
        )}
      </section>
    </div>,
    document.body,
  );
}

function MeetingPipeline({
  stage,
}: {
  stage: "recording" | "transcribing" | "analyzing";
}) {
  const activeIndex =
    stage === "recording" ? 0 : stage === "transcribing" ? 1 : 2;
  const steps = [
    {
      label: copy.meetings.pipelineRecording,
      description: copy.meetings.pipelineRecordingDescription,
    },
    {
      label: copy.meetings.pipelineTranscribing,
      description: copy.meetings.pipelineTranscribingDescription,
    },
    {
      label: copy.meetings.pipelineAnalyzing,
      description: copy.meetings.pipelineAnalyzingDescription,
    },
  ];

  return (
    <ol className="meeting-pipeline" aria-label={copy.meetings.pipelineLabel}>
      {steps.map((step, index) => {
        const state =
          index < activeIndex
            ? "complete"
            : index === activeIndex
              ? "active"
              : "pending";
        return (
          <li
            key={step.label}
            data-state={state}
            aria-current={state === "active" ? "step" : undefined}
          >
            <span aria-hidden="true">
              {state === "complete" ? <Check /> : index + 1}
            </span>
            <div>
              <strong>{step.label}</strong>
              <small>{step.description}</small>
            </div>
          </li>
        );
      })}
    </ol>
  );
}

function MeetingReview({
  detail,
  busyItemId,
  savingItemId,
  bulkApplying,
  retrying,
  deleting,
  onDecide,
  onUpdate,
  onApplyRemaining,
  onRetry,
  onSaveTranscript,
  onReloadTranscript,
  onReanalyzeTranscript,
  onTranscriptDirtyChange,
  onDelete,
}: {
  detail: MeetingDetail;
  busyItemId: string | undefined;
  savingItemId: string | undefined;
  bulkApplying: boolean;
  retrying: boolean;
  deleting: boolean;
  onDecide(item: MeetingActionItem, decision: "approve" | "reject"): void;
  onUpdate(
    item: MeetingActionItem,
    input: Parameters<typeof updateMeetingAction>[4],
  ): Promise<boolean>;
  onApplyRemaining(): void;
  onRetry(): void;
  onSaveTranscript(
    input: MeetingTranscriptUpdateInput,
  ): Promise<MeetingTranscriptUpdateResult>;
  onReloadTranscript(): Promise<MeetingDetail>;
  onReanalyzeTranscript(expectedVersion: number): Promise<void>;
  onTranscriptDirtyChange(dirty: boolean): void;
  onDelete(): Promise<boolean>;
}) {
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const deleteTriggerRef = useRef<HTMLButtonElement>(null);
  const deleteSafeActionRef = useRef<HTMLButtonElement>(null);
  const restoreDeleteTriggerRef = useRef(false);

  useEffect(() => {
    if (confirmingDelete) {
      deleteSafeActionRef.current?.focus();
      return;
    }
    if (restoreDeleteTriggerRef.current) {
      restoreDeleteTriggerRef.current = false;
      deleteTriggerRef.current?.focus();
    }
  }, [confirmingDelete]);

  useEffect(() => {
    if (!confirmingDelete) return;
    return registerMobileBackHandler(() => {
      restoreDeleteTriggerRef.current = true;
      setConfirmingDelete(false);
      return true;
    }, 120);
  }, [confirmingDelete]);

  useEffect(() => {
    if (!confirmingDelete) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      restoreDeleteTriggerRef.current = true;
      setConfirmingDelete(false);
    }
    document.addEventListener("keydown", onKeyDown, true);
    return () => document.removeEventListener("keydown", onKeyDown, true);
  }, [confirmingDelete]);

  if (
    ["recording", "transcribing", "queued", "analyzing"].includes(detail.status)
  ) {
    const transcribing = detail.status === "transcribing";
    const recording = detail.status === "recording";
    const pipelineStage = recording
      ? "recording"
      : transcribing
        ? "transcribing"
        : "analyzing";
    return (
      <div className="meeting-analysis-state" role="status">
        <span className="meeting-analysis-state__mark">
          <LoaderCircle className="spin" aria-hidden="true" />
        </span>
        <div>
          <MeetingStatusLabel status={detail.status} />
          <h2>
            {recording
              ? copy.meetings.recordingQueuedTitle
              : transcribing
                ? copy.meetings.transcribingTitle
                : copy.meetings.analyzingTitle}
          </h2>
          <p>
            {recording
              ? copy.meetings.recordingQueuedDescription
              : transcribing
                ? copy.meetings.transcribingDescription
                : copy.meetings.analyzingDescription}
          </p>
        </div>
        <MeetingPipeline stage={pipelineStage} />
        <div className="meeting-analysis-state__progress" aria-hidden="true">
          <span />
        </div>
      </div>
    );
  }
  const analysisOutdated =
    detail.analyzedAt === null && detail.transcriptSegments.length > 0;
  const visibleActionItems = analysisOutdated
    ? detail.actionItems.filter((item) => item.status === "applied")
    : detail.actionItems;
  const pendingActionCount = visibleActionItems.filter(
    (item) => item.status === "suggested",
  ).length;

  return (
    <article className="meeting-review">
      {(detail.status === "failed" || analysisOutdated) && (
        <section className="meeting-review__analysis-notice" role="alert">
          <CircleAlert aria-hidden="true" />
          <div>
            <h2>
              {detail.status === "failed"
                ? copy.meetings.analysisFailedTitle
                : copy.meetings.transcriptAnalysisOutdatedTitle}
            </h2>
            <p>
              {detail.status === "failed"
                ? copy.meetings.analysisFailedDescription
                : copy.meetings.transcriptAnalysisOutdatedDescription}
            </p>
          </div>
          <button
            className="primary-button"
            type="button"
            disabled={retrying}
            onClick={onRetry}
          >
            {retrying && <LoaderCircle className="spin" aria-hidden="true" />}
            {retrying ? copy.meetings.retrying : copy.meetings.retryAnalysis}
          </button>
        </section>
      )}

      <header className="meeting-review__header">
        <div>
          <MeetingStatusLabel
            status={analysisOutdated ? "failed" : detail.status}
          />
          <h2>{detail.title}</h2>
          <p>
            {detail.projectTitle ?? copy.meetings.noProject} ·{" "}
            {longDate(detail.startedAt ?? detail.createdAt)}
          </p>
          {(detail.purpose || detail.participants.length > 0) && (
            <div className="meeting-review__context">
              {detail.purpose && <span>{detail.purpose}</span>}
              {detail.participants.length > 0 && (
                <span>
                  <Users aria-hidden="true" />
                  {detail.participants.join(", ")}
                </span>
              )}
            </div>
          )}
        </div>
        <div className="meeting-review__header-actions">
          {detail.durationSeconds && (
            <span className="meeting-review__duration">
              <Clock3 aria-hidden="true" />
              {durationLabel(detail.durationSeconds)}
            </span>
          )}
          <button
            ref={deleteTriggerRef}
            className="destructive-quiet-button focus-visible-control"
            type="button"
            disabled={deleting}
            onClick={() => setConfirmingDelete(true)}
          >
            <Trash2 aria-hidden="true" />
            {copy.meetings.deleteMeeting}
          </button>
        </div>
      </header>

      {confirmingDelete && (
        <section
          className="meeting-review__delete-confirmation"
          role="group"
          aria-label={copy.meetings.deleteConfirmTitle}
        >
          <div>
            <strong>{copy.meetings.deleteConfirmTitle}</strong>
            <p>{copy.meetings.deleteConfirmDescription}</p>
          </div>
          <div className="meeting-review__delete-actions">
            <button
              ref={deleteSafeActionRef}
              className="secondary-button focus-visible-control"
              type="button"
              disabled={deleting}
              onClick={() => {
                restoreDeleteTriggerRef.current = true;
                setConfirmingDelete(false);
              }}
            >
              {copy.meetings.keepMeeting}
            </button>
            <button
              className="destructive-button focus-visible-control"
              type="button"
              disabled={deleting}
              onClick={() => void onDelete()}
            >
              {deleting ? (
                <LoaderCircle className="spin" aria-hidden="true" />
              ) : (
                <Trash2 aria-hidden="true" />
              )}
              {deleting
                ? copy.meetings.deletingMeeting
                : copy.meetings.deleteMeeting}
            </button>
          </div>
        </section>
      )}

      {detail.summary && (
        <section className="meeting-review__summary">
          <span>{copy.meetings.summaryLabel}</span>
          <p>{detail.summary}</p>
          {detail.topics.length > 0 && (
            <div className="meeting-review__topics">
              {detail.topics.map((topic) => (
                <span key={topic}>{topic}</span>
              ))}
            </div>
          )}
        </section>
      )}

      {(detail.transcriptSegments.length > 0 || detail.recording?.notes) && (
        <MeetingTranscriptPanel
          meetingId={detail.id}
          meetingVersion={detail.version}
          notes={detail.recording?.notes}
          participants={detail.participants}
          speakers={detail.speakers}
          segments={detail.transcriptSegments}
          onSave={onSaveTranscript}
          onReload={onReloadTranscript}
          onReanalyze={onReanalyzeTranscript}
          onDirtyChange={onTranscriptDirtyChange}
        />
      )}

      <div className="meeting-review__columns">
        <section className="meeting-review__section">
          <div className="meetings-section-heading">
            <h3>
              <Check aria-hidden="true" />
              {copy.meetings.decisionsTitle}
            </h3>
            <span>{copy.meetings.count(detail.decisions.length)}</span>
          </div>
          {detail.decisions.length === 0 ? (
            <p className="meeting-review__empty-copy">
              {copy.meetings.noDecisions}
            </p>
          ) : (
            <ul className="meeting-decision-list">
              {detail.decisions.map((decision) => (
                <li key={decision.id}>
                  <strong>{decision.content}</strong>
                  {decision.rationale && <p>{decision.rationale}</p>}
                  <blockquote>
                    <Quote aria-hidden="true" />
                    {decision.sourceExcerpt}
                  </blockquote>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="meeting-review__section">
          <div className="meetings-section-heading">
            <h3>
              <ListChecks aria-hidden="true" />
              {copy.meetings.actionsTitle}
            </h3>
            <div className="meetings-section-heading__actions">
              <span>{copy.meetings.count(visibleActionItems.length)}</span>
              {pendingActionCount > 1 && (
                <button
                  className="secondary-button"
                  type="button"
                  disabled={bulkApplying || Boolean(busyItemId)}
                  onClick={onApplyRemaining}
                >
                  {bulkApplying && (
                    <LoaderCircle className="spin" aria-hidden="true" />
                  )}
                  {bulkApplying
                    ? copy.meetings.applyingRemaining
                    : copy.meetings.applyRemaining(pendingActionCount)}
                </button>
              )}
            </div>
          </div>
          {visibleActionItems.length === 0 ? (
            <p className="meeting-review__empty-copy">
              {copy.meetings.noActions}
            </p>
          ) : (
            <div className="meeting-action-list">
              {visibleActionItems.map((item) => (
                <MeetingActionCard
                  item={item}
                  busy={busyItemId === item.id}
                  saving={savingItemId === item.id}
                  key={item.id}
                  onDecide={onDecide}
                  onUpdate={onUpdate}
                />
              ))}
            </div>
          )}
        </section>
      </div>

      {(detail.risks.length > 0 || detail.followUp) && (
        <section className="meeting-review__follow-up">
          <CircleAlert aria-hidden="true" />
          <div>
            <h3>{copy.meetings.followUpTitle}</h3>
            {detail.followUp && <p>{detail.followUp}</p>}
            {detail.risks.length > 0 && (
              <ul>
                {detail.risks.map((risk) => (
                  <li key={risk}>{risk}</li>
                ))}
              </ul>
            )}
          </div>
        </section>
      )}
    </article>
  );
}

function MeetingActionCard({
  item,
  busy,
  saving,
  onDecide,
  onUpdate,
}: {
  item: MeetingActionItem;
  busy: boolean;
  saving: boolean;
  onDecide(item: MeetingActionItem, decision: "approve" | "reject"): void;
  onUpdate(
    item: MeetingActionItem,
    input: Parameters<typeof updateMeetingAction>[4],
  ): Promise<boolean>;
}) {
  const pending = item.status === "suggested";
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(item.title);
  const [notes, setNotes] = useState(item.notes ?? "");
  const [assigneeName, setAssigneeName] = useState(item.assigneeName ?? "");
  const [priority, setPriority] = useState(String(item.priority));
  const [dueAt, setDueAt] = useState(datetimeLocalValue(item.dueAt));
  const [startsAt, setStartsAt] = useState(datetimeLocalValue(item.startsAt));
  const [endsAt, setEndsAt] = useState(datetimeLocalValue(item.endsAt));

  useEffect(() => {
    setTitle(item.title);
    setNotes(item.notes ?? "");
    setAssigneeName(item.assigneeName ?? "");
    setPriority(String(item.priority));
    setDueAt(datetimeLocalValue(item.dueAt));
    setStartsAt(datetimeLocalValue(item.startsAt));
    setEndsAt(datetimeLocalValue(item.endsAt));
  }, [item]);

  async function save() {
    const saved = await onUpdate(item, {
      title: title.trim(),
      notes: notes.trim() || undefined,
      assigneeName: assigneeName.trim() || undefined,
      priority: Number(priority),
      dueAt: item.kind === "task" ? isoValue(dueAt) : undefined,
      startsAt: item.kind === "schedule" ? isoValue(startsAt) : undefined,
      endsAt: item.kind === "schedule" ? isoValue(endsAt) : undefined,
      timeZone: item.kind === "schedule" ? "Asia/Seoul" : undefined,
    });
    if (saved) setEditing(false);
  }

  return (
    <article className="meeting-action-card" data-status={item.status}>
      <div className="meeting-action-card__icon" aria-hidden="true">
        {item.kind === "schedule" ? <CalendarPlus /> : <ListChecks />}
      </div>
      <div className="meeting-action-card__content">
        <div className="meeting-action-card__meta">
          <span>
            {item.kind === "schedule"
              ? copy.meetings.scheduleAction
              : copy.meetings.taskAction}
          </span>
          <span>{copy.meetings.confidence(item.confidence)}</span>
        </div>
        <strong>{item.title}</strong>
        {item.notes && <p>{item.notes}</p>}
        {item.assigneeName && (
          <small className="meeting-action-card__assignee">
            {copy.meetings.assignee(item.assigneeName)}
          </small>
        )}
        <small>{actionTimeLabel(item)}</small>
        <blockquote>{item.sourceExcerpt}</blockquote>
      </div>
      {pending && editing && (
        <div className="meeting-action-editor">
          <label>
            <span>{copy.meetings.actionTitleLabel}</span>
            <input
              value={title}
              maxLength={200}
              onChange={(event) => setTitle(event.target.value)}
            />
          </label>
          <label>
            <span>{copy.meetings.actionNotesLabel}</span>
            <textarea
              value={notes}
              maxLength={4_000}
              rows={3}
              onChange={(event) => setNotes(event.target.value)}
            />
          </label>
          <div className="meeting-action-editor__row">
            <label>
              <span>{copy.meetings.assigneeLabel}</span>
              <input
                value={assigneeName}
                maxLength={120}
                placeholder={copy.meetings.assigneePlaceholder}
                onChange={(event) => setAssigneeName(event.target.value)}
              />
            </label>
            <label>
              <span>{copy.meetings.priorityLabel}</span>
              <select
                value={priority}
                onChange={(event) => setPriority(event.target.value)}
              >
                <option value="0">{copy.meetings.priorityOptions.low}</option>
                <option value="1">
                  {copy.meetings.priorityOptions.normal}
                </option>
                <option value="2">{copy.meetings.priorityOptions.high}</option>
                <option value="3">
                  {copy.meetings.priorityOptions.urgent}
                </option>
              </select>
            </label>
          </div>
          {item.kind === "task" ? (
            <label>
              <span>{copy.meetings.dueAtLabel}</span>
              <input
                type="datetime-local"
                value={dueAt}
                onChange={(event) => setDueAt(event.target.value)}
              />
            </label>
          ) : (
            <div className="meeting-action-editor__row">
              <label>
                <span>{copy.meetings.startsAtLabel}</span>
                <input
                  type="datetime-local"
                  value={startsAt}
                  onChange={(event) => setStartsAt(event.target.value)}
                />
              </label>
              <label>
                <span>{copy.meetings.endsAtLabel}</span>
                <input
                  type="datetime-local"
                  value={endsAt}
                  onChange={(event) => setEndsAt(event.target.value)}
                />
              </label>
            </div>
          )}
          <div className="meeting-action-editor__actions">
            <button
              className="secondary-button"
              type="button"
              disabled={saving}
              onClick={() => setEditing(false)}
            >
              {copy.actions.cancel}
            </button>
            <button
              className="primary-button"
              type="button"
              disabled={saving || !title.trim()}
              onClick={() => void save()}
            >
              {saving ? (
                <LoaderCircle className="spin" aria-hidden="true" />
              ) : (
                <Save aria-hidden="true" />
              )}
              {saving ? copy.meetings.savingAction : copy.meetings.saveAction}
            </button>
          </div>
        </div>
      )}
      {pending ? (
        <div className="meeting-action-card__actions">
          <button
            className="secondary-button"
            type="button"
            disabled={busy || saving}
            onClick={() => setEditing((current) => !current)}
          >
            <Pencil aria-hidden="true" />
            {editing ? copy.meetings.closeEdit : copy.meetings.editAction}
          </button>
          <button
            className="secondary-button"
            type="button"
            disabled={busy || saving}
            onClick={() => onDecide(item, "reject")}
          >
            {copy.meetings.exclude}
          </button>
          <button
            className="primary-button"
            type="button"
            disabled={busy || saving || editing}
            onClick={() => onDecide(item, "approve")}
          >
            {busy ? (
              <LoaderCircle className="spin" aria-hidden="true" />
            ) : item.kind === "schedule" ? (
              <CalendarPlus aria-hidden="true" />
            ) : (
              <FolderKanban aria-hidden="true" />
            )}
            {copy.meetings.apply}
          </button>
        </div>
      ) : (
        <span className="meeting-action-card__result">
          {item.status === "applied"
            ? copy.meetings.applied
            : copy.meetings.excluded}
        </span>
      )}
    </article>
  );
}

function MeetingStatusLabel({ status }: { status: Meeting["status"] }) {
  return (
    <span className="meeting-status" data-status={status}>
      {copy.meetings.status[status]}
    </span>
  );
}

function EmptyMeetings({ onCreate }: { onCreate(): void }) {
  return (
    <div className="meetings-list__empty">
      <FileAudio aria-hidden="true" />
      <strong>{copy.meetings.emptyTitle}</strong>
      <p>{copy.meetings.emptyDescription}</p>
      <button className="secondary-button" type="button" onClick={onCreate}>
        <Plus aria-hidden="true" />
        {copy.meetings.newMeeting}
      </button>
    </div>
  );
}

function MeetingDetailSkeleton({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="meeting-detail__skeleton"
      label={copy.meetings.loading}
      visible={visible}
    >
      <SkeletonBlock />
      <SkeletonBlock />
      <SkeletonBlock />
      <SkeletonBlock />
    </SkeletonGroup>
  );
}

function preferredRecordingMimeType(): string | undefined {
  return [
    "audio/webm;codecs=opus",
    "audio/mp4;codecs=mp4a.40.2",
    "audio/mp4",
    "audio/webm",
  ].find((mimeType) => MediaRecorder.isTypeSupported(mimeType));
}

export function meetingRecordingAudioConstraints(): MediaTrackConstraints {
  return {
    // Meeting capture needs the original voice characteristics for speaker
    // embeddings. Browser call-processing can suppress a distant attendee or
    // a voice played through the device speaker as echo/noise.
    echoCancellation: false,
    noiseSuppression: false,
    autoGainControl: false,
    channelCount: 1,
  };
}

function recordingTime(seconds: number): string {
  const hours = Math.floor(seconds / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  const remainder = seconds % 60;
  return [hours, minutes, remainder]
    .map((value) => String(value).padStart(2, "0"))
    .join(":");
}

function notesSaveLabel(state: "idle" | "saving" | "saved" | "failed"): string {
  return {
    idle: copy.meetings.notesReady,
    saving: copy.meetings.notesSaving,
    saved: copy.meetings.notesSaved,
    failed: copy.meetings.notesSaveFailed,
  }[state];
}

function shortDate(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
  }).format(new Date(value));
}

function longDate(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    year: "numeric",
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function durationLabel(seconds: number): string {
  const minutes = Math.max(1, Math.round(seconds / 60));
  return minutes >= 60
    ? `${Math.floor(minutes / 60)}시간 ${minutes % 60}분`
    : `${minutes}분`;
}

function actionTimeLabel(item: MeetingActionItem): string {
  const value = item.kind === "schedule" ? item.startsAt : item.dueAt;
  if (!value) return copy.meetings.timeNotSet;
  return new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function normalizedParticipants(value: string): string[] {
  return Array.from(
    new Set(
      value
        .split(/[,\n]/)
        .map((participant) => participant.trim())
        .filter(Boolean),
    ),
  ).slice(0, 100);
}

function datetimeLocalValue(value: string | null): string {
  if (!value) return "";
  const date = new Date(value);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

function isoValue(value: string): string | undefined {
  if (!value) return undefined;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
}
