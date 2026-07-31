import {
  Check,
  Eye,
  ExternalLink,
  Inbox,
  Link2,
  LoaderCircle,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";

import {
  type GoogleChatAccount,
  type GoogleChatSpace,
  type ProjectGoogleChatSource,
  type ProjectInflowItem,
  type ProjectInflowPromotionBlockReason,
  type ProjectInflowReferenceDocument,
  projectInflowPromotionReadiness,
} from "../api/googleChat";
import { copy } from "../copy";
import {
  DeadlinePicker,
  isoToSeoulLocalDateTime,
  seoulLocalDateTimeToIso,
} from "./DeadlinePicker";

type ProjectInflowPanelProps = {
  accountsAvailable: boolean;
  accounts: GoogleChatAccount[];
  spaces: GoogleChatSpace[];
  sources: ProjectGoogleChatSource[];
  items: ProjectInflowItem[];
  loading: boolean;
  saving: boolean;
  problemMessage?: string;
  onConnectAccount(): Promise<void>;
  onLoadSpaces(accountId: string): Promise<void>;
  onCreateSource(input: {
    accountId: string;
    spaceName: string;
    displayName: string;
    acknowledgeWithReaction: boolean;
    importHistory: boolean;
  }): Promise<void>;
  onDeleteSource(source: ProjectGoogleChatSource): Promise<void>;
  onSyncSource(source: ProjectGoogleChatSource): Promise<void>;
  onPromote(item: ProjectInflowItem, input: PromoteInflowInput): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
  onRetryAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryCompletion(item: ProjectInflowItem): Promise<void>;
  onOpenTask(taskId: string): void;
};

export type PromoteInflowInput = {
  title: string;
  notes: string;
  assigneeName?: string;
  priority: number;
  dueAt: string | null;
  withoutDeadline: boolean;
};

export type InflowDraftField =
  "title" | "notes" | "assigneeName" | "priority" | "dueAt" | "withoutDeadline";

export type InflowDraftValues = {
  title: string;
  notes: string;
  assigneeName: string;
  priority: string;
  dueAt: string;
  withoutDeadline: boolean;
};

type PersistedInflowDraft = {
  savedAt: number;
  baseRevision: number;
  dirtyFields: InflowDraftField[];
} & InflowDraftValues;

export function ProjectInflowPanel({
  accountsAvailable,
  accounts,
  spaces,
  sources,
  items,
  loading,
  saving,
  problemMessage,
  onConnectAccount,
  onLoadSpaces,
  onCreateSource,
  onDeleteSource,
  onSyncSource,
  onPromote,
  onDismiss,
  onRetryAnalysis,
  onRetryCompletion,
  onOpenTask,
}: ProjectInflowPanelProps) {
  const activeAccounts = useMemo(
    () => accounts.filter((account) => account.status === "active"),
    [accounts],
  );
  const [accountId, setAccountId] = useState("");
  const [spaceName, setSpaceName] = useState("");
  const [acknowledge, setAcknowledge] = useState(true);
  const [importHistory, setImportHistory] = useState(false);
  const pendingItems = items.filter(isProjectInflowAttentionItem);
  const handledItems = items
    .filter((item) => item.status !== "pending")
    .slice(0, 12);
  const reconnectRequired =
    sources.length > 0 &&
    accounts.some((account) => account.status === "reauth_required") &&
    activeAccounts.length === 0;

  useEffect(() => {
    const next = activeAccounts.some((account) => account.id === accountId)
      ? accountId
      : (activeAccounts[0]?.id ?? "");
    if (next !== accountId) setAccountId(next);
    if (next) void onLoadSpaces(next);
  }, [accountId, activeAccounts, onLoadSpaces]);

  useEffect(() => {
    if (!spaces.some((space) => space.name === spaceName)) setSpaceName("");
  }, [spaceName, spaces]);

  async function addSource(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const space = spaces.find((item) => item.name === spaceName);
    if (!accountId || !space) return;
    await onCreateSource({
      accountId,
      spaceName: space.name,
      displayName: space.displayName,
      acknowledgeWithReaction: acknowledge,
      importHistory,
    });
    setSpaceName("");
  }

  return (
    <section className="project-inflow" aria-labelledby="project-inflow-title">
      <header className="project-inflow__heading">
        <div className="project-inflow__heading-icon" aria-hidden="true">
          <Inbox />
        </div>
        <div>
          <h3 id="project-inflow-title">{copy.projects.inflowTitle}</h3>
          <p>{copy.projects.inflowDescription}</p>
        </div>
      </header>

      {problemMessage && !reconnectRequired && (
        <p className="inline-alert" role="alert">
          {problemMessage}
        </p>
      )}

      {activeAccounts.length === 0 ? (
        <div
          className="project-inflow__connect"
          data-state={reconnectRequired ? "reauth-required" : "disconnected"}
          role={reconnectRequired ? "alert" : undefined}
        >
          <div>
            <strong>
              {reconnectRequired
                ? copy.projects.inflowReconnectTitle
                : copy.projects.inflowConnectAccount}
            </strong>
            <p>
              {reconnectRequired
                ? copy.projects.inflowReconnectProblem
                : copy.projects.inflowConnectDescription}
            </p>
          </div>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={!accountsAvailable || saving}
            onClick={() => void onConnectAccount()}
          >
            <Link2 aria-hidden="true" />
            {reconnectRequired
              ? copy.projects.inflowReconnectAction
              : copy.projects.inflowConnectAccount}
          </button>
        </div>
      ) : (
        <form
          className="project-inflow__source-form"
          onSubmit={(event) => void addSource(event)}
        >
          <label>
            <span>{copy.projects.inflowAccountLabel}</span>
            <select
              value={accountId}
              disabled={saving}
              onChange={(event) => {
                setAccountId(event.target.value);
                setSpaceName("");
              }}
            >
              {activeAccounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.email}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>{copy.projects.inflowSpaceLabel}</span>
            <select
              value={spaceName}
              disabled={loading || saving}
              onChange={(event) => setSpaceName(event.target.value)}
            >
              <option value="">{copy.projects.inflowChooseSpace}</option>
              {spaces.map((space) => (
                <option key={space.name} value={space.name}>
                  {space.displayName}
                </option>
              ))}
            </select>
          </label>
          <div className="project-inflow__source-form-actions">
            <label className="project-inflow__acknowledge">
              <input
                type="checkbox"
                checked={acknowledge}
                disabled={saving}
                onChange={(event) => setAcknowledge(event.target.checked)}
              />
              <span>{copy.projects.inflowAckLabel}</span>
            </label>
            <label className="project-inflow__acknowledge">
              <input
                type="checkbox"
                checked={importHistory}
                disabled={saving}
                onChange={(event) => setImportHistory(event.target.checked)}
              />
              <span>{copy.projects.inflowImportHistoryLabel}</span>
            </label>
            <button
              className="secondary-button focus-visible-control"
              type="submit"
              disabled={!spaceName || saving}
            >
              {saving ? (
                <LoaderCircle className="spin" aria-hidden="true" />
              ) : (
                <Link2 aria-hidden="true" />
              )}
              {copy.projects.inflowAddSource}
            </button>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={!accountsAvailable || saving}
              onClick={() => void onConnectAccount()}
            >
              <Link2 aria-hidden="true" />
              {copy.projects.inflowConnectAnotherAccount}
            </button>
          </div>
        </form>
      )}

      {sources.length > 0 && (
        <ul className="project-inflow__sources" aria-label="연결된 Chat 공간">
          {sources.map((source) => (
            <li key={source.id}>
              <div>
                <strong>{source.displayName}</strong>
                <span>{source.accountEmail}</span>
              </div>
              <div className="project-inflow__source-actions">
                {source.acknowledgeWithReaction && (
                  <span className="project-inflow__ack-state">
                    <Eye aria-hidden="true" /> 확인 표시
                  </span>
                )}
                <button
                  className="icon-button focus-visible-control"
                  type="button"
                  aria-label={`${source.displayName} ${copy.projects.inflowRefresh}`}
                  disabled={loading || saving}
                  onClick={() => void onSyncSource(source)}
                >
                  <RefreshCw aria-hidden="true" />
                </button>
                <button
                  className="icon-button focus-visible-control"
                  type="button"
                  aria-label={`${source.displayName} ${copy.projects.inflowRemoveSource}`}
                  disabled={saving}
                  onClick={() => void onDeleteSource(source)}
                >
                  <Trash2 aria-hidden="true" />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {sources.length === 0 && activeAccounts.length > 0 && (
        <p className="project-inflow__empty">{copy.projects.inflowNoSource}</p>
      )}

      {sources.length > 0 && (
        <div className="project-inflow__items" aria-busy={loading}>
          {loading && items.length === 0 ? (
            <p className="project-inflow__empty">
              <LoaderCircle className="spin" aria-hidden="true" /> 새 메시지를
              확인하고 있어요.
            </p>
          ) : items.length === 0 ? (
            <p className="project-inflow__empty">{copy.projects.inflowEmpty}</p>
          ) : (
            <>
              {pendingItems.length === 0 && (
                <p className="project-inflow__empty project-inflow__empty--pending">
                  {copy.projects.inflowEmpty}
                </p>
              )}
              <InflowItemList
                title={copy.projects.inflowPendingTitle}
                items={pendingItems}
                saving={saving}
                onPromote={onPromote}
                onDismiss={onDismiss}
                onRetryAnalysis={onRetryAnalysis}
                onRetryCompletion={onRetryCompletion}
                onOpenTask={onOpenTask}
              />
              {handledItems.length > 0 && (
                <details className="project-inflow__history">
                  <summary className="focus-visible-control">
                    <span>
                      <strong>{copy.projects.inflowRecentTitle}</strong>
                      <small>{handledItems.length}건</small>
                    </span>
                    <span className="project-inflow__history-open">
                      {copy.projects.inflowHistoryOpen}
                    </span>
                    <span className="project-inflow__history-close">
                      {copy.projects.inflowHistoryClose}
                    </span>
                  </summary>
                  <InflowItemList
                    title={copy.projects.inflowRecentTitle}
                    items={handledItems}
                    saving={saving}
                    onPromote={onPromote}
                    onDismiss={onDismiss}
                    onRetryAnalysis={onRetryAnalysis}
                    onRetryCompletion={onRetryCompletion}
                    onOpenTask={onOpenTask}
                    hideHeading
                  />
                </details>
              )}
            </>
          )}
        </div>
      )}
    </section>
  );
}

function InflowItemList({
  title,
  items,
  saving,
  onPromote,
  onDismiss,
  onRetryAnalysis,
  onRetryCompletion,
  onOpenTask,
  hideHeading = false,
}: {
  title: string;
  items: ProjectInflowItem[];
  saving: boolean;
  onPromote(item: ProjectInflowItem, input: PromoteInflowInput): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
  onRetryAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryCompletion(item: ProjectInflowItem): Promise<void>;
  onOpenTask(taskId: string): void;
  hideHeading?: boolean;
}) {
  if (items.length === 0) return null;
  return (
    <section
      className="project-inflow__group"
      {...(hideHeading
        ? { "aria-label": title }
        : { "aria-labelledby": `inflow-${title}` })}
    >
      {!hideHeading && <h4 id={`inflow-${title}`}>{title}</h4>}
      <ul>
        {items.map((item) => (
          <InflowItemRow
            key={inflowConversationKey(item)}
            item={item}
            saving={saving}
            onPromote={onPromote}
            onDismiss={onDismiss}
            onRetryAnalysis={onRetryAnalysis}
            onRetryCompletion={onRetryCompletion}
            onOpenTask={onOpenTask}
          />
        ))}
      </ul>
    </section>
  );
}

export function InflowItemRow({
  item,
  saving,
  onPromote,
  onDismiss,
  onRetryAnalysis,
  onRetryCompletion,
  onOpenTask,
}: {
  item: ProjectInflowItem;
  saving: boolean;
  onPromote(item: ProjectInflowItem, input: PromoteInflowInput): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
  onRetryAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryCompletion(item: ProjectInflowItem): Promise<void>;
  onOpenTask?(taskId: string): void;
}) {
  const conversationId = inflowConversationKey(item);
  const restoredDraft = useMemo(
    () => readInflowDraft(conversationId),
    [conversationId],
  );
  const [editing, setEditing] = useState(Boolean(restoredDraft));
  const [promoting, setPromoting] = useState(false);
  const [promotionError, setPromotionError] = useState<string>();
  const messages = item.messages ?? [
    {
      senderName: item.senderName,
      contentText: item.contentText,
      receivedAt: item.receivedAt,
    },
  ];
  const suggestedTitle =
    item.suggestedTaskTitle || "대화를 업무로 정리하고 있어요";
  const referenceLinks = item.referenceLinks ?? [];
  const referenceDocuments = item.referenceDocuments ?? [];
  const messageCount = item.messageCount ?? messages.length;
  const firstReceivedAt = item.firstReceivedAt ?? item.receivedAt;
  const assigneeOptions = useMemo(
    () => item.assigneeOptions ?? [],
    [item.assigneeOptions],
  );
  const suggestedAssignee =
    item.suggestedAssigneeName &&
    assigneeOptions.includes(item.suggestedAssigneeName)
      ? item.suggestedAssigneeName
      : "";
  const sourceRevision = item.sourceRevision ?? 0;
  const analyzedRevision = item.analyzedRevision;
  const [title, setTitle] = useState(
    () => restoredDraft?.title ?? suggestedTitle,
  );
  const [notes, setNotes] = useState(
    () => restoredDraft?.notes ?? item.suggestedTaskNotes,
  );
  const [assigneeName, setAssigneeName] = useState(
    () => restoredDraft?.assigneeName ?? suggestedAssignee,
  );
  const [dueAt, setDueAt] = useState(
    () => restoredDraft?.dueAt ?? isoToSeoulLocalDateTime(item.suggestedDueAt),
  );
  const [withoutDeadline, setWithoutDeadline] = useState(
    () => restoredDraft?.withoutDeadline ?? false,
  );
  const [dueProblem, setDueProblem] = useState(false);
  const [priority, setPriority] = useState(
    () => restoredDraft?.priority ?? String(item.suggestedPriority ?? 1),
  );
  const dirtyFieldsRef = useRef(
    new Set<InflowDraftField>(restoredDraft?.dirtyFields ?? []),
  );
  const [draftBaseRevision, setDraftBaseRevision] = useState(
    () => restoredDraft?.baseRevision ?? analyzedRevision ?? sourceRevision,
  );
  const [contextOpen, setContextOpen] = useState(false);
  const contextSummaryRef = useRef<HTMLElement | null>(null);
  const contextFocusFrameRef = useRef<number | undefined>(undefined);
  const analysisReady = item.analysisStatus === "ready";
  const analysisFailed = item.analysisStatus === "failed";
  const analysisRefreshing = item.analysisStatus === "refreshing";
  const analysisStale = item.analysisStatus === "stale";
  const hasUsableAnalysis =
    analysisReady || analysisRefreshing || analysisStale;
  const hasNewReplies = editing && sourceRevision > draftBaseRevision;
  const newReplyCount = Math.max(1, sourceRevision - draftBaseRevision);
  const existingTaskFollowUp = isExistingTaskFollowUp(item);
  const promotionReadiness = projectInflowPromotionReadiness(item);
  const promotionProblem = promotionReadiness.canPromote
    ? undefined
    : projectInflowPromotionProblem(promotionReadiness.reason);
  const canNotifyAssignee = Boolean(
    assigneeName && item.notifiableAssigneeNames?.includes(assigneeName),
  );

  useEffect(() => {
    if (!hasUsableAnalysis) return;
    const dirtyFields = dirtyFieldsRef.current;
    const merged = mergeInflowDraftValues(
      { title, notes, assigneeName, priority, dueAt, withoutDeadline },
      {
        title: suggestedTitle,
        notes: item.suggestedTaskNotes,
        assigneeName: suggestedAssignee,
        dueAt: isoToSeoulLocalDateTime(item.suggestedDueAt),
        withoutDeadline: false,
        priority: String(item.suggestedPriority ?? 1),
      },
      editing ? dirtyFields : [],
    );
    setTitle(merged.title);
    setNotes(merged.notes);
    setAssigneeName(merged.assigneeName);
    setDueAt(merged.dueAt);
    setWithoutDeadline(merged.withoutDeadline);
    setPriority(merged.priority);
    if (!editing) dirtyFields.clear();
    setDraftBaseRevision((current) =>
      nextInflowDraftBaseRevision(current, analyzedRevision, editing),
    );
  }, [
    analyzedRevision,
    editing,
    hasUsableAnalysis,
    assigneeName,
    dueAt,
    item.suggestedDueAt,
    item.suggestedPriority,
    item.suggestedTaskNotes,
    notes,
    priority,
    suggestedAssignee,
    suggestedTitle,
    title,
    withoutDeadline,
  ]);

  useEffect(() => {
    if (item.status !== "pending" || item.promotedTaskId) {
      clearInflowDraft(conversationId);
      return;
    }
    if (!editing) return;
    writeInflowDraft(conversationId, {
      savedAt: Date.now(),
      baseRevision: draftBaseRevision,
      title,
      notes,
      assigneeName,
      priority,
      dueAt,
      withoutDeadline,
      dirtyFields: [...dirtyFieldsRef.current],
    });
  }, [
    assigneeName,
    conversationId,
    draftBaseRevision,
    dueAt,
    editing,
    item.promotedTaskId,
    item.status,
    notes,
    priority,
    title,
    withoutDeadline,
  ]);

  useEffect(
    () => () => {
      if (contextFocusFrameRef.current !== undefined) {
        window.cancelAnimationFrame(contextFocusFrameRef.current);
      }
    },
    [],
  );

  function markDirty(field: InflowDraftField) {
    dirtyFieldsRef.current.add(field);
  }

  function openConversationContext() {
    setContextOpen(true);
    if (contextFocusFrameRef.current !== undefined) {
      window.cancelAnimationFrame(contextFocusFrameRef.current);
    }
    const reduceMotion =
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    contextFocusFrameRef.current = window.requestAnimationFrame(() => {
      contextFocusFrameRef.current = undefined;
      contextSummaryRef.current?.focus({ preventScroll: true });
      contextSummaryRef.current?.scrollIntoView({
        block: "nearest",
        behavior: inflowContextScrollBehavior(reduceMotion),
      });
    });
  }

  async function dismissItem() {
    await onDismiss(item);
    clearInflowDraft(conversationId);
  }

  async function submitPromotion(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim() || promoting || saving) return;
    if (!promotionReadiness.canPromote) {
      setPromotionError(promotionProblem);
      return;
    }
    const deadline = resolvePromotionDeadline(dueAt, withoutDeadline);
    if (!deadline) {
      setDueProblem(true);
      setPromotionError(copy.projects.inflowDueAtProblem);
      document.getElementById(`inflow-due-${item.id}-date`)?.focus();
      return;
    }
    setDueProblem(false);
    setPromoting(true);
    setPromotionError(undefined);
    try {
      await onPromote(item, {
        title: title.trim(),
        notes: notes.trim(),
        assigneeName: assigneeName || undefined,
        priority: Number(priority),
        ...deadline,
      });
      setEditing(false);
      clearInflowDraft(conversationId);
    } catch {
      setPromotionError(copy.projects.inflowDecisionProblem);
    } finally {
      setPromoting(false);
    }
  }

  return (
    <li
      className="project-inflow-item"
      id={`project-inflow-item-${item.id}`}
      tabIndex={-1}
    >
      <div className="project-inflow-item__meta">
        <span>{item.projectName}</span>
        <span>{item.sourceName}</span>
        <span>대화 {messageCount}개</span>
        <span>{formatConversationRange(firstReceivedAt, item.receivedAt)}</span>
        {item.acknowledged && <span>👀 표시 완료</span>}
      </div>
      <div className="project-inflow-item__summary">
        <strong>{suggestedTitle}</strong>
        <p>
          {item.analysisSummary ??
            (analysisFailed
              ? copy.projects.inflowAnalysisHelp
              : copy.projects.inflowAnalyzing)}
        </p>
        {item.analysisConfidence !== null && hasUsableAnalysis && (
          <span>
            {copy.projects.inflowAnalysisSummary} · 확신도{" "}
            {item.analysisConfidence}%
          </span>
        )}
      </div>
      {referenceLinks.length > 0 && (
        <div className="project-inflow-item__references">
          <span>
            <Link2 aria-hidden="true" />
            관련 링크
          </span>
          <ul>
            {referenceLinks.map((link) => (
              <li key={link}>
                <a href={link} target="_blank" rel="noreferrer">
                  {link}
                </a>
              </li>
            ))}
          </ul>
        </div>
      )}
      {referenceDocuments.length > 0 && (
        <ReferenceEvidence documents={referenceDocuments} />
      )}
      {messages.length > 0 && (
        <details
          className="project-inflow-item__context"
          open={contextOpen}
          onToggle={(event) => setContextOpen(event.currentTarget.open)}
        >
          <summary ref={contextSummaryRef}>
            원문 대화 {messages.length}개 보기
          </summary>
          <ol>
            {messages.map((message, index) => (
              <li key={`${message.receivedAt}-${index}`}>
                <div>
                  <strong>
                    {message.senderName ?? copy.projects.inflowSenderPending}
                  </strong>
                  <time dateTime={message.receivedAt}>
                    {formatReceivedAt(message.receivedAt)}
                  </time>
                </div>
                <p>{message.contentText}</p>
              </li>
            ))}
          </ol>
        </details>
      )}
      {existingTaskFollowUp ? (
        <div className="project-inflow-item__follow-up" role="group">
          <div>
            <strong>{copy.projects.inflowFollowUpTitle}</strong>
            <p>{copy.projects.inflowFollowUpDescription}</p>
          </div>
          <div className="project-inflow-item__follow-up-actions">
            {onOpenTask && (
              <button
                className="primary-button focus-visible-control"
                type="button"
                disabled={saving}
                onClick={() => onOpenTask(item.promotedTaskId!)}
              >
                <Eye aria-hidden="true" />
                {copy.projects.inflowFollowUpOpenTask}
              </button>
            )}
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => void dismissItem()}
            >
              <Check aria-hidden="true" />
              {copy.projects.inflowFollowUpDone}
            </button>
          </div>
        </div>
      ) : item.status !== "pending" ? (
        <div
          className={`project-inflow-item__completion project-inflow-item__completion--${item.status}`}
          role="status"
        >
          <strong>
            {item.status === "promoted"
              ? copy.projects.inflowPromoted
              : copy.projects.inflowDismissed}
          </strong>
          {item.status === "promoted" && (
            <>
              <p>
                {item.completionStatus === "sent"
                  ? copy.projects.inflowCompletionSent
                  : item.completionStatus === "failed"
                    ? copy.projects.inflowCompletionRetrying
                    : copy.projects.inflowCompletionPending}
              </p>
              <div>
                {item.completionReactionCompleted && (
                  <span>{copy.projects.inflowReactionDone}</span>
                )}
                {item.completionReplyCompleted && (
                  <span>{copy.projects.inflowReplyDone}</span>
                )}
              </div>
              {item.completionStatus !== "sent" && (
                <button
                  className="secondary-button focus-visible-control"
                  type="button"
                  disabled={saving}
                  onClick={() => void onRetryCompletion(item)}
                >
                  <RefreshCw aria-hidden="true" />
                  {copy.projects.inflowCompletionRetry}
                </button>
              )}
            </>
          )}
        </div>
      ) : analysisFailed && !editing ? (
        <div className="project-inflow-item__analysis-state" role="status">
          <p>{copy.projects.inflowAnalysisHelp}</p>
          <div>
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => void onRetryAnalysis(item)}
            >
              <RefreshCw aria-hidden="true" />
              {copy.projects.inflowAnalysisRetry}
            </button>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => void dismissItem()}
            >
              <X aria-hidden="true" /> {copy.projects.inflowDismiss}
            </button>
          </div>
        </div>
      ) : !hasUsableAnalysis && !editing ? (
        <div
          className="project-inflow-item__analysis-state"
          role="status"
          aria-live="polite"
        >
          <p>
            <LoaderCircle className="spin" aria-hidden="true" />
            {copy.projects.inflowAnalyzing}
          </p>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => void dismissItem()}
          >
            <X aria-hidden="true" /> {copy.projects.inflowDismiss}
          </button>
        </div>
      ) : !promotionReadiness.canPromote && !editing ? (
        <div className="project-inflow-item__analysis-state" role="status">
          <p>{promotionProblem}</p>
          <div>
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => void onRetryAnalysis(item)}
            >
              <RefreshCw aria-hidden="true" />
              {copy.projects.inflowAnalysisRetry}
            </button>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => void dismissItem()}
            >
              <X aria-hidden="true" /> {copy.projects.inflowDismiss}
            </button>
          </div>
        </div>
      ) : editing ? (
        <form
          className="project-inflow-item__promote"
          onSubmit={(event) => void submitPromotion(event)}
        >
          {promotionProblem && (
            <div
              className="project-inflow-item__analysis-state"
              id={`inflow-promotion-problem-${item.id}`}
              role="status"
            >
              <p>{promotionProblem}</p>
              <div>
                <button
                  className="secondary-button focus-visible-control"
                  type="button"
                  disabled={saving}
                  onClick={() => void onRetryAnalysis(item)}
                >
                  <RefreshCw aria-hidden="true" />
                  {copy.projects.inflowAnalysisRetry}
                </button>
              </div>
            </div>
          )}
          {hasNewReplies && (
            <div
              className="project-inflow-item__revision-alert"
              role="status"
              aria-live="polite"
            >
              <div>
                <strong>
                  {copy.projects.inflowNewRepliesTitle(newReplyCount)}
                </strong>
                <p>
                  {analysisRefreshing
                    ? copy.projects.inflowNewRepliesRefreshing
                    : analysisStale
                      ? copy.projects.inflowNewRepliesStale
                      : copy.projects.inflowNewRepliesDescription}
                </p>
              </div>
              <div>
                <button
                  className="secondary-button focus-visible-control"
                  type="button"
                  disabled={saving}
                  onClick={openConversationContext}
                >
                  {copy.projects.inflowNewRepliesOpen}
                </button>
                {!analysisRefreshing && (
                  <button
                    className="secondary-button focus-visible-control"
                    type="button"
                    disabled={saving}
                    onClick={() => void onRetryAnalysis(item)}
                  >
                    <RefreshCw aria-hidden="true" />
                    {copy.projects.inflowNewRepliesApply}
                  </button>
                )}
                <button
                  className="text-button focus-visible-control"
                  type="button"
                  disabled={saving}
                  onClick={() => setDraftBaseRevision(sourceRevision)}
                >
                  {copy.projects.inflowNewRepliesKeepDraft}
                </button>
              </div>
            </div>
          )}
          <div className="project-inflow-item__fields">
            <label className="project-inflow-item__title-field">
              <span>{copy.projects.inflowTaskTitleLabel}</span>
              <input
                value={title}
                maxLength={300}
                disabled={saving}
                aria-describedby={`inflow-task-title-help-${item.id}`}
                onChange={(event) => {
                  markDirty("title");
                  setTitle(event.target.value);
                }}
              />
              <small id={`inflow-task-title-help-${item.id}`}>
                {copy.projects.inflowTaskTitleHint}
              </small>
            </label>
            <label className="project-inflow-item__notes-field">
              <span>{copy.projects.inflowTaskNotesLabel}</span>
              <textarea
                value={notes}
                maxLength={10_000}
                rows={8}
                disabled={saving}
                aria-describedby={`inflow-task-notes-help-${item.id}`}
                onChange={(event) => {
                  markDirty("notes");
                  setNotes(event.target.value);
                }}
              />
              <small id={`inflow-task-notes-help-${item.id}`}>
                {copy.projects.inflowTaskNotesHint}
              </small>
            </label>
            <label>
              <span>{copy.projects.inflowAssigneeLabel}</span>
              <select
                value={assigneeName}
                disabled={saving}
                onChange={(event) => {
                  markDirty("assigneeName");
                  setAssigneeName(event.target.value);
                }}
              >
                <option value="">{copy.projects.inflowNoAssignee}</option>
                {assigneeOptions.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
            <div className="project-inflow-item__deadline-field">
              <DeadlinePicker
                id={`inflow-due-${item.id}`}
                label={copy.projects.inflowDueAtLabel}
                value={dueAt}
                disabled={saving || withoutDeadline}
                invalid={dueProblem}
                describedBy={
                  dueProblem ? `inflow-due-problem-${item.id}` : undefined
                }
                showPresets
                allowClear={false}
                onChange={(value) => {
                  markDirty("dueAt");
                  setDueAt(value);
                  setDueProblem(false);
                  setPromotionError(undefined);
                }}
              />
              {dueProblem && (
                <small id={`inflow-due-problem-${item.id}`} role="alert">
                  {copy.projects.inflowDueAtProblem}
                </small>
              )}
            </div>
            <label className="project-inflow-item__no-deadline">
              <input
                type="checkbox"
                checked={withoutDeadline}
                disabled={saving}
                onChange={(event) => {
                  markDirty("withoutDeadline");
                  setWithoutDeadline(event.currentTarget.checked);
                  setDueProblem(false);
                  setPromotionError(undefined);
                }}
              />
              <span>{copy.projects.inflowWithoutDeadline}</span>
            </label>
            <label>
              <span>{copy.projects.inflowPriorityLabel}</span>
              <select
                value={priority}
                disabled={saving}
                onChange={(event) => {
                  markDirty("priority");
                  setPriority(event.target.value);
                }}
              >
                <option value="1">{copy.forms.priorityNormal}</option>
                <option value="2">{copy.forms.priorityImportant}</option>
                <option value="3">{copy.forms.priorityHighest}</option>
              </select>
            </label>
          </div>
          {promotionError && (
            <p
              className="assistant-inline-alert"
              role="alert"
              aria-live="assertive"
            >
              {promotionError}
            </p>
          )}
          {assigneeName && (
            <p className="project-inflow-item__notification-note">
              {canNotifyAssignee
                ? copy.projects.inflowAssigneeWillBeNotified(assigneeName)
                : copy.projects.inflowAssigneeNotificationOff}
            </p>
          )}
          <div>
            <button
              className="primary-button focus-visible-control"
              type="submit"
              disabled={
                !title.trim() ||
                saving ||
                promoting ||
                !promotionReadiness.canPromote
              }
              aria-describedby={
                promotionProblem
                  ? `inflow-promotion-problem-${item.id}`
                  : undefined
              }
            >
              {promoting ? (
                <span className="button-spinner" aria-hidden="true" />
              ) : (
                <Check aria-hidden="true" />
              )}
              {promoting
                ? copy.projects.inflowPromoting
                : canNotifyAssignee
                  ? copy.projects.inflowPromoteAndNotify
                  : copy.projects.inflowPromote}
            </button>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={saving || promoting}
              onClick={() => {
                setPromotionError(undefined);
                setEditing(false);
                dirtyFieldsRef.current.clear();
                clearInflowDraft(conversationId);
              }}
            >
              <X aria-hidden="true" /> 취소
            </button>
          </div>
        </form>
      ) : (
        <div className="project-inflow-item__actions">
          <button
            className="primary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => {
              setPromotionError(undefined);
              setDraftBaseRevision(analyzedRevision ?? sourceRevision);
              setEditing(true);
            }}
          >
            <Check aria-hidden="true" /> {copy.projects.inflowPromote}
          </button>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => void dismissItem()}
          >
            <X aria-hidden="true" /> {copy.projects.inflowDismiss}
          </button>
        </div>
      )}
    </li>
  );
}

export function isProjectInflowAttentionItem(item: ProjectInflowItem): boolean {
  return item.status === "pending";
}

export function projectInflowAttentionCount(
  items: ProjectInflowItem[],
): number {
  return items.filter(isProjectInflowAttentionItem).length;
}

export function isExistingTaskFollowUp(item: ProjectInflowItem): boolean {
  return item.status === "pending" && Boolean(item.promotedTaskId);
}

export function inflowConversationKey(item: ProjectInflowItem): string {
  return item.conversationId ?? item.id;
}

export function mergeInflowDraftValues(
  current: InflowDraftValues,
  suggested: InflowDraftValues,
  dirtyFields: Iterable<InflowDraftField>,
): InflowDraftValues {
  const dirty = new Set(dirtyFields);
  return {
    title: dirty.has("title") ? current.title : suggested.title,
    notes: dirty.has("notes") ? current.notes : suggested.notes,
    assigneeName: dirty.has("assigneeName")
      ? current.assigneeName
      : suggested.assigneeName,
    priority: dirty.has("priority") ? current.priority : suggested.priority,
    dueAt: dirty.has("dueAt") ? current.dueAt : suggested.dueAt,
    withoutDeadline: dirty.has("withoutDeadline")
      ? current.withoutDeadline
      : suggested.withoutDeadline,
  };
}

function ReferenceEvidence({
  documents,
}: {
  documents: ProjectInflowReferenceDocument[];
}) {
  return (
    <details className="project-inflow-item__evidence">
      <summary>{copy.projects.inflowEvidenceSummary(documents.length)}</summary>
      <p>{copy.projects.inflowEvidenceDescription}</p>
      <ul>
        {documents.map((document) => {
          const externalUrl = safeExternalUrl(document.url);
          return (
            <li key={`${document.provider}:${document.externalId}`}>
              <header>
                <div>
                  <strong>
                    {document.title ||
                      copy.projects.inflowEvidenceUntitled(document.externalId)}
                  </strong>
                  <span>
                    {document.provider} · {document.externalId}
                  </span>
                </div>
                {externalUrl && (
                  <a href={externalUrl} target="_blank" rel="noreferrer">
                    {copy.projects.inflowEvidenceOpen}
                    <ExternalLink aria-hidden="true" />
                  </a>
                )}
              </header>
              {document.originalContent ? (
                <pre>{document.originalContent}</pre>
              ) : (
                <p>{copy.projects.inflowEvidenceUnavailable}</p>
              )}
            </li>
          );
        })}
      </ul>
    </details>
  );
}

export function projectInflowPromotionProblem(
  reason: ProjectInflowPromotionBlockReason,
): string {
  switch (reason) {
    case "not_actionable":
      return copy.projects.inflowPromotionNotActionable;
    case "missing_context":
      return copy.projects.inflowPromotionContextMissing;
    case "analysis_stale":
      return copy.projects.inflowPromotionStale;
    default:
      return copy.projects.inflowPromotionNotReady;
  }
}

function safeExternalUrl(value: string): string | undefined {
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:"
      ? url.toString()
      : undefined;
  } catch {
    return undefined;
  }
}

export function nextInflowDraftBaseRevision(
  currentRevision: number,
  analyzedRevision: number | null,
  editing: boolean,
): number {
  if (editing || analyzedRevision === null) return currentRevision;
  return Math.max(currentRevision, analyzedRevision);
}

export function inflowContextScrollBehavior(
  reduceMotion: boolean,
): ScrollBehavior {
  return reduceMotion ? "auto" : "smooth";
}

export function localInputToIso(value: string): string | undefined {
  return seoulLocalDateTimeToIso(value);
}

export function resolvePromotionDeadline(
  value: string,
  withoutDeadline: boolean,
): Pick<PromoteInflowInput, "dueAt" | "withoutDeadline"> | undefined {
  if (withoutDeadline) {
    return { dueAt: null, withoutDeadline: true };
  }
  const dueAt = localInputToIso(value);
  return dueAt ? { dueAt, withoutDeadline: false } : undefined;
}

const INFLOW_DRAFT_PREFIX = "jimin-os:inflow-draft:";
const INFLOW_DRAFT_MAX_AGE = 24 * 60 * 60 * 1_000;

function readInflowDraft(
  conversationId: string,
): PersistedInflowDraft | undefined {
  if (typeof window === "undefined") return undefined;
  try {
    const raw = window.sessionStorage.getItem(
      `${INFLOW_DRAFT_PREFIX}${conversationId}`,
    );
    if (!raw) return undefined;
    const value = JSON.parse(raw) as Partial<PersistedInflowDraft>;
    if (
      typeof value.savedAt !== "number" ||
      Date.now() - value.savedAt > INFLOW_DRAFT_MAX_AGE ||
      typeof value.baseRevision !== "number" ||
      typeof value.title !== "string" ||
      typeof value.notes !== "string" ||
      typeof value.assigneeName !== "string" ||
      typeof value.priority !== "string" ||
      typeof value.dueAt !== "string" ||
      typeof value.withoutDeadline !== "boolean" ||
      !Array.isArray(value.dirtyFields)
    ) {
      clearInflowDraft(conversationId);
      return undefined;
    }
    return value as PersistedInflowDraft;
  } catch {
    clearInflowDraft(conversationId);
    return undefined;
  }
}

function writeInflowDraft(conversationId: string, draft: PersistedInflowDraft) {
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage.setItem(
      `${INFLOW_DRAFT_PREFIX}${conversationId}`,
      JSON.stringify(draft),
    );
  } catch {
    // The active in-memory draft remains available when storage is blocked.
  }
}

function clearInflowDraft(conversationId: string) {
  if (typeof window === "undefined") return;
  try {
    window.sessionStorage.removeItem(`${INFLOW_DRAFT_PREFIX}${conversationId}`);
  } catch {
    // Storage can be unavailable in privacy-restricted webviews.
  }
}

function formatReceivedAt(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "받은 시간 확인 필요";
  return new Intl.DateTimeFormat("ko-KR", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function formatConversationRange(
  firstValue: string,
  lastValue: string,
): string {
  const first = new Date(firstValue);
  const last = new Date(lastValue);
  if (Number.isNaN(first.getTime()) || Number.isNaN(last.getTime())) {
    return "받은 시간 확인 필요";
  }
  const formatter = new Intl.DateTimeFormat("ko-KR", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
  if (first.getTime() === last.getTime()) return formatter.format(last);
  return `${formatter.format(first)}–${formatter.format(last)}`;
}
