import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  CalendarPlus,
  Check,
  ChevronRight,
  CircleAlert,
  Clock3,
  FileAudio,
  FolderKanban,
  ListChecks,
  LoaderCircle,
  Mic,
  Plus,
  Quote,
  Square,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import {
  createMeeting,
  decideMeetingAction,
  fetchMeeting,
  fetchMeetings,
  reanalyzeMeeting,
  type Meeting,
  type MeetingActionItem,
  type MeetingDetail,
  type MeetingSummary,
} from "../api/meetings";
import { type Project, type Workspace } from "../api/projects";
import { copy } from "../copy";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";

type MeetingsWorkspaceProps = {
  apiBaseUrl: string;
  accessToken: string;
  workspaces: Workspace[];
  projects: Project[];
  selectedWorkspaceId: string | undefined;
  onSelectWorkspace(workspaceId: string): void;
};

type NativeVoiceResult = { transcript: string };
type RecognitionResultLike = {
  isFinal: boolean;
  0: { transcript: string };
};
type RecognitionEventLike = {
  resultIndex: number;
  results: ArrayLike<RecognitionResultLike>;
};
type RecognitionErrorLike = { error: string };
type RecognitionLike = {
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  onresult: ((event: RecognitionEventLike) => void) | null;
  onerror: ((event: RecognitionErrorLike) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
};
type RecognitionConstructor = new () => RecognitionLike;

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
  const [retrying, setRetrying] = useState(false);
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
        setError(copy.meetings.detailFailed);
      } finally {
        if (!quiet) setDetailLoading(false);
      }
    },
    [accessToken, apiBaseUrl],
  );

  useEffect(() => {
    void loadList();
  }, [loadList]);

  useEffect(() => {
    if (!selectedMeetingId) {
      setDetail(undefined);
      return;
    }
    void loadDetail(selectedMeetingId);
  }, [loadDetail, selectedMeetingId]);

  useEffect(() => {
    if (!detail || !["queued", "analyzing"].includes(detail.status)) return;
    const timer = window.setInterval(() => {
      void loadDetail(detail.id, true);
    }, 1_800);
    return () => window.clearInterval(timer);
  }, [detail, loadDetail]);

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
      await reanalyzeMeeting(apiBaseUrl, accessToken, detail.id);
      await loadDetail(detail.id, true);
    } catch {
      setError(copy.meetings.retryFailed);
    } finally {
      setRetrying(false);
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

      <div className="meetings-layout">
        <aside className="meetings-list" aria-label={copy.meetings.listLabel}>
          <div className="meetings-section-heading">
            <h2>{copy.meetings.recent}</h2>
            <span>{copy.meetings.count(meetings.length)}</span>
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
            <div className="meetings-list__items">
              {meetings.map((meeting) => (
                <button
                  className="meeting-list-item focus-visible-control"
                  data-active={meeting.id === selectedMeetingId}
                  type="button"
                  key={meeting.id}
                  onClick={() => setSelectedMeetingId(meeting.id)}
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
              detail={detail}
              busyItemId={decisionBusyId}
              retrying={retrying}
              onDecide={decide}
              onRetry={retryAnalysis}
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
          workspaces={workspaces}
          projects={projects}
          selectedWorkspaceId={selectedWorkspaceId}
          saving={creating}
          onSelectWorkspace={onSelectWorkspace}
          onClose={() => setShowComposer(false)}
          onSubmit={submitMeeting}
        />
      )}
    </section>
  );
}

type MeetingComposerInput = Parameters<typeof createMeeting>[2];

function MeetingComposer({
  workspaces,
  projects,
  selectedWorkspaceId,
  saving,
  onSelectWorkspace,
  onClose,
  onSubmit,
}: {
  workspaces: Workspace[];
  projects: Project[];
  selectedWorkspaceId: string | undefined;
  saving: boolean;
  onSelectWorkspace(workspaceId: string): void;
  onClose(): void;
  onSubmit(input: MeetingComposerInput): Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [projectId, setProjectId] = useState("");
  const [transcript, setTranscript] = useState("");
  const [dictating, setDictating] = useState(false);
  const [dictationError, setDictationError] = useState<string>();
  const recognition = useRef<RecognitionLike | null>(null);
  const dictatingRef = useRef(false);
  const startedAt = useRef<Date | null>(null);

  const stopDictation = useCallback(() => {
    dictatingRef.current = false;
    recognition.current?.stop();
    recognition.current = null;
    if (usesAndroidNativeRecognition()) {
      void invoke("plugin:voice-recognition|cancel").catch(() => undefined);
    }
    setDictating(false);
  }, []);

  useEffect(() => stopDictation, [stopDictation]);

  function appendTranscript(value: string) {
    const text = value.trim();
    if (!text) return;
    setTranscript(
      (current) => `${current.trim()}${current.trim() ? "\n" : ""}${text}`,
    );
  }

  async function startNativeDictation() {
    while (dictatingRef.current) {
      try {
        const result = await invoke<NativeVoiceResult>(
          "plugin:voice-recognition|start",
        );
        if (!dictatingRef.current) return;
        appendTranscript(result.transcript);
      } catch {
        if (dictatingRef.current) {
          setDictationError(copy.meetings.dictationFailed);
          stopDictation();
        }
      }
    }
  }

  function startBrowserDictation(Constructor: RecognitionConstructor) {
    const recognizer = new Constructor();
    recognition.current = recognizer;
    recognizer.lang = "ko-KR";
    recognizer.interimResults = false;
    recognizer.continuous = true;
    recognizer.onresult = (event) => {
      for (
        let index = event.resultIndex;
        index < event.results.length;
        index += 1
      ) {
        const result = event.results[index];
        if (result?.isFinal) appendTranscript(result[0]?.transcript ?? "");
      }
    };
    recognizer.onerror = (event) => {
      if (event.error === "no-speech") return;
      setDictationError(copy.meetings.dictationFailed);
      stopDictation();
    };
    recognizer.onend = () => {
      if (!dictatingRef.current) return;
      try {
        recognizer.start();
      } catch {
        setDictationError(copy.meetings.dictationFailed);
        stopDictation();
      }
    };
    recognizer.start();
  }

  function startDictation() {
    setDictationError(undefined);
    dictatingRef.current = true;
    startedAt.current ??= new Date();
    setDictating(true);
    if (usesAndroidNativeRecognition()) {
      void startNativeDictation();
      return;
    }
    const Constructor = recognitionConstructor();
    if (!Constructor) {
      setDictationError(copy.meetings.dictationUnsupported);
      stopDictation();
      return;
    }
    try {
      startBrowserDictation(Constructor);
    } catch {
      setDictationError(copy.meetings.dictationPermission);
      stopDictation();
    }
  }

  async function submit() {
    stopDictation();
    const started = startedAt.current;
    await onSubmit({
      title: title.trim(),
      transcript: transcript.trim(),
      workspaceId: selectedWorkspaceId,
      projectId: projectId || undefined,
      startedAt: started?.toISOString(),
      durationSeconds: started
        ? Math.max(1, Math.round((Date.now() - started.getTime()) / 1_000))
        : undefined,
    });
  }

  const canSubmit = title.trim().length > 0 && transcript.trim().length > 0;

  return createPortal(
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="meeting-composer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="meeting-composer-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <span>{copy.meetings.composerEyebrow}</span>
            <h2 id="meeting-composer-title">{copy.meetings.composerTitle}</h2>
            <p>{copy.meetings.composerDescription}</p>
          </div>
          <button
            className="icon-button focus-visible-control"
            type="button"
            aria-label={copy.actions.cancel}
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </header>

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

        <div className="meeting-composer__dictation" data-active={dictating}>
          <button
            className="dictation-button focus-visible-control"
            type="button"
            onClick={dictating ? stopDictation : startDictation}
          >
            {dictating ? (
              <Square aria-hidden="true" />
            ) : (
              <Mic aria-hidden="true" />
            )}
            {dictating
              ? copy.meetings.stopDictation
              : copy.meetings.startDictation}
          </button>
          <div>
            <strong>
              {dictating
                ? copy.meetings.dictatingTitle
                : copy.meetings.dictationTitle}
            </strong>
            <p>
              {dictating
                ? copy.meetings.dictatingDescription
                : copy.meetings.dictationDescription}
            </p>
          </div>
        </div>
        {dictationError && (
          <p className="meeting-composer__error" role="alert">
            {dictationError}
          </p>
        )}

        <footer>
          <button className="secondary-button" type="button" onClick={onClose}>
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
        </footer>
      </section>
    </div>,
    document.body,
  );
}

function MeetingReview({
  detail,
  busyItemId,
  retrying,
  onDecide,
  onRetry,
}: {
  detail: MeetingDetail;
  busyItemId: string | undefined;
  retrying: boolean;
  onDecide(item: MeetingActionItem, decision: "approve" | "reject"): void;
  onRetry(): void;
}) {
  if (["queued", "analyzing"].includes(detail.status)) {
    return (
      <div className="meeting-analysis-state" role="status">
        <span className="meeting-analysis-state__mark">
          <LoaderCircle className="spin" aria-hidden="true" />
        </span>
        <div>
          <MeetingStatusLabel status={detail.status} />
          <h2>{copy.meetings.analyzingTitle}</h2>
          <p>{copy.meetings.analyzingDescription}</p>
        </div>
        <div className="meeting-analysis-state__progress" aria-hidden="true">
          <span />
        </div>
      </div>
    );
  }
  if (detail.status === "failed") {
    return (
      <div className="meeting-detail__empty" role="alert">
        <CircleAlert aria-hidden="true" />
        <h2>{copy.meetings.analysisFailedTitle}</h2>
        <p>{copy.meetings.analysisFailedDescription}</p>
        <button
          className="primary-button"
          type="button"
          disabled={retrying}
          onClick={onRetry}
        >
          {retrying && <LoaderCircle className="spin" aria-hidden="true" />}
          {retrying ? copy.meetings.retrying : copy.meetings.retryAnalysis}
        </button>
      </div>
    );
  }

  return (
    <article className="meeting-review">
      <header className="meeting-review__header">
        <div>
          <MeetingStatusLabel status={detail.status} />
          <h2>{detail.title}</h2>
          <p>
            {detail.projectTitle ?? copy.meetings.noProject} ·{" "}
            {longDate(detail.startedAt ?? detail.createdAt)}
          </p>
        </div>
        {detail.durationSeconds && (
          <span className="meeting-review__duration">
            <Clock3 aria-hidden="true" />
            {durationLabel(detail.durationSeconds)}
          </span>
        )}
      </header>

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
            <span>{copy.meetings.count(detail.actionItems.length)}</span>
          </div>
          {detail.actionItems.length === 0 ? (
            <p className="meeting-review__empty-copy">
              {copy.meetings.noActions}
            </p>
          ) : (
            <div className="meeting-action-list">
              {detail.actionItems.map((item) => (
                <MeetingActionCard
                  item={item}
                  busy={busyItemId === item.id}
                  key={item.id}
                  onDecide={onDecide}
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
  onDecide,
}: {
  item: MeetingActionItem;
  busy: boolean;
  onDecide(item: MeetingActionItem, decision: "approve" | "reject"): void;
}) {
  const pending = item.status === "suggested";
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
        <small>{actionTimeLabel(item)}</small>
        <blockquote>{item.sourceExcerpt}</blockquote>
      </div>
      {pending ? (
        <div className="meeting-action-card__actions">
          <button
            className="secondary-button"
            type="button"
            disabled={busy}
            onClick={() => onDecide(item, "reject")}
          >
            {copy.meetings.exclude}
          </button>
          <button
            className="primary-button"
            type="button"
            disabled={busy}
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

function recognitionConstructor(): RecognitionConstructor | undefined {
  const source = window as typeof window & {
    SpeechRecognition?: RecognitionConstructor;
    webkitSpeechRecognition?: RecognitionConstructor;
  };
  return source.SpeechRecognition ?? source.webkitSpeechRecognition;
}

function usesAndroidNativeRecognition(): boolean {
  return isTauri() && /Android/i.test(navigator.userAgent);
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
