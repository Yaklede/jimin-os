import {
  Check,
  Eye,
  Inbox,
  Link2,
  LoaderCircle,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";

import {
  type GoogleChatAccount,
  type GoogleChatSpace,
  type ProjectGoogleChatSource,
  type ProjectInflowItem,
} from "../api/googleChat";
import { copy } from "../copy";

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
};

export type PromoteInflowInput = {
  title: string;
  notes: string;
  assigneeName?: string;
  priority: number;
  dueAt?: string;
};

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
}: ProjectInflowPanelProps) {
  const activeAccounts = useMemo(
    () => accounts.filter((account) => account.status === "active"),
    [accounts],
  );
  const [accountId, setAccountId] = useState("");
  const [spaceName, setSpaceName] = useState("");
  const [acknowledge, setAcknowledge] = useState(true);
  const [importHistory, setImportHistory] = useState(false);
  const pendingItems = items.filter((item) => item.status === "pending");
  const handledItems = items
    .filter((item) => item.status !== "pending")
    .slice(0, 12);

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

      {problemMessage && (
        <p className="inline-alert" role="alert">
          {problemMessage}
        </p>
      )}

      {activeAccounts.length === 0 ? (
        <div className="project-inflow__connect">
          <div>
            <strong>{copy.projects.inflowConnectAccount}</strong>
            <p>{copy.projects.inflowConnectDescription}</p>
          </div>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={!accountsAvailable || saving}
            onClick={() => void onConnectAccount()}
          >
            <Link2 aria-hidden="true" />
            {copy.projects.inflowConnectAccount}
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
              <InflowItemList
                title={copy.projects.inflowPendingTitle}
                items={pendingItems}
                saving={saving}
                onPromote={onPromote}
                onDismiss={onDismiss}
                onRetryAnalysis={onRetryAnalysis}
                onRetryCompletion={onRetryCompletion}
              />
              <InflowItemList
                title={copy.projects.inflowRecentTitle}
                items={handledItems}
                saving={saving}
                onPromote={onPromote}
                onDismiss={onDismiss}
                onRetryAnalysis={onRetryAnalysis}
                onRetryCompletion={onRetryCompletion}
              />
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
}: {
  title: string;
  items: ProjectInflowItem[];
  saving: boolean;
  onPromote(item: ProjectInflowItem, input: PromoteInflowInput): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
  onRetryAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryCompletion(item: ProjectInflowItem): Promise<void>;
}) {
  if (items.length === 0) return null;
  return (
    <section
      className="project-inflow__group"
      aria-labelledby={`inflow-${title}`}
    >
      <h4 id={`inflow-${title}`}>{title}</h4>
      <ul>
        {items.map((item) => (
          <InflowItemRow
            key={item.id}
            item={item}
            saving={saving}
            onPromote={onPromote}
            onDismiss={onDismiss}
            onRetryAnalysis={onRetryAnalysis}
            onRetryCompletion={onRetryCompletion}
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
}: {
  item: ProjectInflowItem;
  saving: boolean;
  onPromote(item: ProjectInflowItem, input: PromoteInflowInput): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
  onRetryAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryCompletion(item: ProjectInflowItem): Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const messages = item.messages ?? [
    {
      senderName: item.senderName,
      contentText: item.contentText,
      receivedAt: item.receivedAt,
    },
  ];
  const suggestedTitle =
    item.suggestedTaskTitle || "대화를 업무로 정리하고 있어요";
  const messageCount = item.messageCount ?? messages.length;
  const firstReceivedAt = item.firstReceivedAt ?? item.receivedAt;
  const [title, setTitle] = useState(() => suggestedTitle);
  const [notes, setNotes] = useState(() => item.suggestedTaskNotes);
  const assigneeOptions = useMemo(
    () => item.assigneeOptions ?? [],
    [item.assigneeOptions],
  );
  const [assigneeName, setAssigneeName] = useState(() =>
    item.suggestedAssigneeName &&
    assigneeOptions.includes(item.suggestedAssigneeName)
      ? item.suggestedAssigneeName
      : "",
  );
  const [dueAt, setDueAt] = useState(() =>
    isoToLocalInput(item.suggestedDueAt),
  );
  const [dueProblem, setDueProblem] = useState(false);
  const [priority, setPriority] = useState(() =>
    String(item.suggestedPriority ?? 1),
  );
  const analysisReady = item.analysisStatus === "ready";
  const analysisFailed = item.analysisStatus === "failed";
  const canNotifyAssignee = Boolean(
    assigneeName && item.notifiableAssigneeNames?.includes(assigneeName),
  );

  useEffect(() => {
    if (editing || !analysisReady) return;
    setTitle(suggestedTitle);
    setNotes(item.suggestedTaskNotes);
    setAssigneeName(
      item.suggestedAssigneeName &&
        assigneeOptions.includes(item.suggestedAssigneeName)
        ? item.suggestedAssigneeName
        : "",
    );
    setDueAt(isoToLocalInput(item.suggestedDueAt));
    setPriority(String(item.suggestedPriority ?? 1));
  }, [
    analysisReady,
    assigneeOptions,
    editing,
    item.suggestedAssigneeName,
    item.suggestedDueAt,
    item.suggestedPriority,
    item.suggestedTaskNotes,
    suggestedTitle,
  ]);

  async function submitPromotion(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim()) return;
    const rawDueAt = String(
      new FormData(event.currentTarget).get("dueAt") ?? "",
    );
    const parsedDueAt = localInputToIso(rawDueAt);
    if (rawDueAt && !parsedDueAt) {
      setDueProblem(true);
      return;
    }
    setDueProblem(false);
    await onPromote(item, {
      title: title.trim(),
      notes: notes.trim(),
      assigneeName: assigneeName || undefined,
      priority: Number(priority),
      dueAt: parsedDueAt,
    });
    setEditing(false);
  }

  return (
    <li className="project-inflow-item">
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
        {item.analysisConfidence !== null && analysisReady && (
          <span>
            {copy.projects.inflowAnalysisSummary} · 확신도{" "}
            {item.analysisConfidence}%
          </span>
        )}
      </div>
      {messages.length > 0 && (
        <details className="project-inflow-item__context">
          <summary>원문 대화 {messages.length}개 보기</summary>
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
      {item.status !== "pending" ? (
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
      ) : analysisFailed ? (
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
              onClick={() => void onDismiss(item)}
            >
              <X aria-hidden="true" /> {copy.projects.inflowDismiss}
            </button>
          </div>
        </div>
      ) : !analysisReady ? (
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
            onClick={() => void onDismiss(item)}
          >
            <X aria-hidden="true" /> {copy.projects.inflowDismiss}
          </button>
        </div>
      ) : editing ? (
        <form
          className="project-inflow-item__promote"
          onSubmit={(event) => void submitPromotion(event)}
        >
          <div className="project-inflow-item__fields">
            <label className="project-inflow-item__title-field">
              <span>{copy.projects.inflowTaskTitleLabel}</span>
              <input
                value={title}
                maxLength={300}
                disabled={saving}
                aria-describedby={`inflow-task-title-help-${item.id}`}
                onChange={(event) => setTitle(event.target.value)}
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
                onChange={(event) => setNotes(event.target.value)}
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
                onChange={(event) => setAssigneeName(event.target.value)}
              >
                <option value="">{copy.projects.inflowNoAssignee}</option>
                {assigneeOptions.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>{copy.projects.inflowDueAtLabel}</span>
              <input
                type="datetime-local"
                name="dueAt"
                value={dueAt}
                disabled={saving}
                aria-invalid={dueProblem}
                aria-describedby={
                  dueProblem ? `inflow-due-problem-${item.id}` : undefined
                }
                onChange={(event) => {
                  setDueAt(event.target.value);
                  setDueProblem(false);
                }}
              />
              {dueProblem && (
                <small id={`inflow-due-problem-${item.id}`} role="alert">
                  {copy.projects.inflowDueAtProblem}
                </small>
              )}
            </label>
            <label>
              <span>{copy.projects.inflowPriorityLabel}</span>
              <select
                value={priority}
                disabled={saving}
                onChange={(event) => setPriority(event.target.value)}
              >
                <option value="1">{copy.forms.priorityNormal}</option>
                <option value="2">{copy.forms.priorityImportant}</option>
                <option value="3">{copy.forms.priorityHighest}</option>
              </select>
            </label>
          </div>
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
              disabled={!title.trim() || saving}
            >
              <Check aria-hidden="true" />
              {canNotifyAssignee
                ? copy.projects.inflowPromoteAndNotify
                : copy.projects.inflowPromote}
            </button>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => setEditing(false)}
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
            onClick={() => setEditing(true)}
          >
            <Check aria-hidden="true" /> {copy.projects.inflowPromote}
          </button>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => void onDismiss(item)}
          >
            <X aria-hidden="true" /> {copy.projects.inflowDismiss}
          </button>
        </div>
      )}
    </li>
  );
}

export function localInputToIso(value: string): string | undefined {
  if (!value) return undefined;
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? undefined : parsed.toISOString();
}

function isoToLocalInput(value: string | null): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return local.toISOString().slice(0, 16);
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
