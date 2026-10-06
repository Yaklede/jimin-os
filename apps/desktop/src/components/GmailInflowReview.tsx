import {
  Building2,
  Clock3,
  ExternalLink,
  Inbox,
  Link2,
  LoaderCircle,
  Mail,
  RefreshCw,
  UserRound,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";

import type { GmailInflowCandidate } from "../api/gmailInflow";
import { PlanningRequestError } from "../api/planning";
import type { Project } from "../api/projects";
import { copy } from "../copy";
import { deadlinePickerCopy } from "../copy/deadlinePicker";
import {
  DeadlinePicker,
  isoToSeoulLocalDateTime,
  resolveOptionalSeoulDateTime,
  seoulLocalDateTimeToIso,
} from "./DeadlinePicker";
import { LinkifiedText, SafeExternalLink } from "./ExternalTextLink";

export interface PromoteGmailInflowInput {
  title: string;
  notes: string;
  projectId: string;
  assigneeName: string | null;
  priority: number;
  dueAt: string | null;
  withoutDeadline: boolean;
}

export type GmailInflowDraftValues = {
  title: string;
  notes: string;
  assigneeName: string;
  priority: number;
  dueAt: string;
};

export type GmailInflowDraftField = keyof GmailInflowDraftValues;

export type GmailInflowReviewProps = {
  items: GmailInflowCandidate[];
  loading: boolean;
  loadingMore: boolean;
  loadMoreError: boolean;
  hasMore: boolean;
  error: string | undefined;
  savingId: string | undefined;
  projects: Project[];
  onReload(): void | Promise<void>;
  onLoadMore(): void | Promise<void>;
  onPromote(
    candidate: GmailInflowCandidate,
    input: PromoteGmailInflowInput,
  ): Promise<void>;
  onDismiss(candidate: GmailInflowCandidate): Promise<void>;
  onDefer(candidate: GmailInflowCandidate, revisitAt: string): Promise<void>;
  onRetryAnalysis(candidate: GmailInflowCandidate): Promise<void>;
  onOpenTask(taskId: string): Promise<void>;
};

export function GmailInflowReview({
  items,
  loading,
  loadingMore,
  loadMoreError,
  hasMore,
  error,
  savingId,
  projects,
  onReload,
  onLoadMore,
  onPromote,
  onDismiss,
  onDefer,
  onRetryAnalysis,
  onOpenTask,
}: GmailInflowReviewProps) {
  const [selectedId, setSelectedId] = useState<string>();
  const pendingItems = useMemo(
    () =>
      items.filter(
        (item) =>
          item.status !== "promoted" &&
          item.status !== "dismissed" &&
          (item.analysisStatus === "ready" || item.analysisStatus === "failed"),
      ),
    [items],
  );
  const selectedItem =
    pendingItems.find((item) => item.id === selectedId) ?? pendingItems[0];

  useEffect(() => {
    if (!selectedItem) {
      setSelectedId(undefined);
      return;
    }
    if (selectedId !== selectedItem.id) setSelectedId(selectedItem.id);
  }, [selectedId, selectedItem]);

  if (loading && pendingItems.length === 0) {
    return (
      <section
        className="gmail-inflow gmail-inflow--state"
        aria-labelledby="gmail-inflow-title"
        aria-busy="true"
      >
        <LoaderCircle className="gmail-inflow__spinner" aria-hidden="true" />
        <div>
          <h2 id="gmail-inflow-title">{copy.gmailInflow.title}</h2>
          <p>{copy.gmailInflow.loading}</p>
        </div>
      </section>
    );
  }

  if (error && pendingItems.length === 0 && !hasMore) {
    return (
      <section
        className="gmail-inflow gmail-inflow--state"
        aria-labelledby="gmail-inflow-title"
      >
        <Inbox aria-hidden="true" />
        <div>
          <h2 id="gmail-inflow-title">{copy.gmailInflow.title}</h2>
          <p role="alert">{error}</p>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            onClick={() => void onReload()}
          >
            <RefreshCw aria-hidden="true" />
            {copy.gmailInflow.reload}
          </button>
        </div>
      </section>
    );
  }

  if (!selectedItem) {
    return (
      <section
        className="gmail-inflow gmail-inflow--state"
        aria-labelledby="gmail-inflow-title"
      >
        <Mail aria-hidden="true" />
        <div>
          <h2 id="gmail-inflow-title">{copy.gmailInflow.emptyTitle}</h2>
          <p>{copy.gmailInflow.emptyDescription}</p>
          {error && !loadMoreError && (
            <div className="gmail-inflow__empty-alert" role="alert">
              <span>{error}</span>
              <button
                className="text-button focus-visible-control"
                type="button"
                onClick={() => void onReload()}
              >
                {copy.gmailInflow.reload}
              </button>
            </div>
          )}
          {loadMoreError && hasMore && (
            <p className="gmail-inflow__empty-alert" role="alert">
              {copy.gmailInflow.moreLoadProblem}
            </p>
          )}
          {hasMore && (
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={loadingMore}
              onClick={() => void onLoadMore()}
            >
              {loadingMore
                ? copy.gmailInflow.loadingMore
                : copy.gmailInflow.loadMore}
            </button>
          )}
        </div>
      </section>
    );
  }

  return (
    <section
      className="gmail-inflow"
      aria-labelledby="gmail-inflow-title"
      aria-busy={loading || loadingMore}
    >
      <header className="gmail-inflow__heading">
        <div>
          <span>{copy.gmailInflow.eyebrow}</span>
          <h2 id="gmail-inflow-title">{copy.gmailInflow.title}</h2>
          <p>{copy.gmailInflow.description}</p>
          <small>{copy.gmailInflow.initialScope}</small>
        </div>
        <strong>{copy.gmailInflow.count(pendingItems.length)}</strong>
      </header>

      {error && (
        <div className="gmail-inflow__partial-alert" role="status">
          <span>
            {loadMoreError ? copy.gmailInflow.moreLoadProblem : error}
          </span>
          <button
            className="text-button focus-visible-control"
            type="button"
            onClick={() => void (loadMoreError ? onLoadMore() : onReload())}
          >
            {loadMoreError
              ? copy.gmailInflow.retryLoadMore
              : copy.gmailInflow.reload}
          </button>
        </div>
      )}

      <div className="gmail-inflow__layout">
        <aside
          className="gmail-inflow__queue"
          aria-labelledby="gmail-inflow-queue-title"
        >
          <div className="gmail-inflow__queue-heading">
            <Mail aria-hidden="true" />
            <strong id="gmail-inflow-queue-title">
              {copy.gmailInflow.queueTitle}
            </strong>
          </div>
          <ol>
            {pendingItems.map((item) => {
              const active = item.id === selectedItem.id;
              return (
                <li key={item.id}>
                  <button
                    className="gmail-inflow__queue-item focus-visible-control"
                    type="button"
                    aria-pressed={active}
                    data-active={active}
                    disabled={Boolean(savingId)}
                    onClick={() => setSelectedId(item.id)}
                  >
                    <span className="gmail-inflow__queue-meta">
                      <WorkspaceBadge item={item} />
                      <time dateTime={item.receivedAt}>
                        {formatReceivedAt(item.receivedAt, true)}
                      </time>
                    </span>
                    <strong>{subjectLabel(item)}</strong>
                    <span>{senderLabel(item)}</span>
                  </button>
                </li>
              );
            })}
          </ol>
          {hasMore && (
            <div className="gmail-inflow__load-more">
              <span>{copy.gmailInflow.moreAvailable}</span>
              <button
                className="secondary-button focus-visible-control"
                type="button"
                disabled={loadingMore || Boolean(savingId)}
                onClick={() => void onLoadMore()}
              >
                {loadingMore
                  ? copy.gmailInflow.loadingMore
                  : copy.gmailInflow.loadMore}
              </button>
            </div>
          )}
        </aside>

        <GmailInflowDetail
          key={selectedItem.id}
          item={selectedItem}
          projects={projects.filter(
            (project) =>
              project.workspaceId === selectedItem.workspaceId &&
              project.status !== "completed",
          )}
          saving={Boolean(savingId)}
          onPromote={onPromote}
          onDismiss={onDismiss}
          onDefer={onDefer}
          onRetryAnalysis={onRetryAnalysis}
          onOpenTask={onOpenTask}
        />
      </div>
    </section>
  );
}

type GmailInflowDetailProps = {
  item: GmailInflowCandidate;
  projects: Project[];
  saving: boolean;
  onPromote(
    candidate: GmailInflowCandidate,
    input: PromoteGmailInflowInput,
  ): Promise<void>;
  onDismiss(candidate: GmailInflowCandidate): Promise<void>;
  onDefer(candidate: GmailInflowCandidate, revisitAt: string): Promise<void>;
  onRetryAnalysis(candidate: GmailInflowCandidate): Promise<void>;
  onOpenTask(taskId: string): Promise<void>;
};

function GmailInflowDetail({
  item,
  projects,
  saving,
  onPromote,
  onDismiss,
  onDefer,
  onRetryAnalysis,
  onOpenTask,
}: GmailInflowDetailProps) {
  const [draft, setDraft] = useState<GmailInflowDraftValues>(() =>
    suggestedGmailInflowDraft(item),
  );
  const [projectId, setProjectId] = useState(
    projects.length === 1 ? (projects[0]?.id ?? "") : "",
  );
  const [revisitAt, setRevisitAt] = useState(defaultRevisitAtLocal);
  const [actionError, setActionError] = useState<string>();
  const dirtyFieldsRef = useRef(new Set<GmailInflowDraftField>());
  const analysisReady = item.analysisStatus === "ready";
  const { title, notes, assigneeName, priority, dueAt } = draft;

  useEffect(() => {
    setDraft((current) =>
      mergeGmailInflowDraftValues(
        current,
        suggestedGmailInflowDraft(item),
        dirtyFieldsRef.current,
      ),
    );
  }, [
    item.suggestedAssigneeName,
    item.suggestedDueAt,
    item.suggestedPriority,
    item.suggestedTaskNotes,
    item.suggestedTaskTitle,
  ]);

  function changeDraft<Field extends GmailInflowDraftField>(
    field: Field,
    value: GmailInflowDraftValues[Field],
  ) {
    dirtyFieldsRef.current.add(field);
    setDraft((current) => ({ ...current, [field]: value }));
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (item.promotedTaskId) return;
    const trimmedTitle = title.trim();
    if (!trimmedTitle) {
      setActionError(copy.gmailInflow.invalidTitle);
      return;
    }
    if (!projectId) {
      setActionError(copy.gmailInflow.invalidProject);
      return;
    }
    const deadline = resolveOptionalSeoulDateTime(dueAt);
    if (!deadline.valid) {
      setActionError(deadlinePickerCopy.invalid);
      document.getElementById(`gmail-inflow-due-at-${item.id}-date`)?.focus();
      return;
    }
    setActionError(undefined);
    try {
      await onPromote(item, {
        title: trimmedTitle,
        notes: notes.trim(),
        projectId,
        assigneeName: assigneeName.trim() || null,
        priority,
        dueAt: deadline.value ?? null,
        withoutDeadline: deadline.value === undefined,
      });
    } catch (error) {
      setActionError(actionFailureMessage(error));
    }
  }

  async function decide(action: "dismiss" | "defer") {
    setActionError(undefined);
    try {
      if (action === "dismiss") await onDismiss(item);
      else {
        const resolvedRevisitAt = deferDateTimeToIso(revisitAt);
        if (!resolvedRevisitAt) {
          setActionError(copy.gmailInflow.invalidDeferAt);
          return;
        }
        await onDefer(item, resolvedRevisitAt);
      }
    } catch (error) {
      setActionError(actionFailureMessage(error));
    }
  }

  async function retryAnalysis() {
    setActionError(undefined);
    try {
      await onRetryAnalysis(item);
    } catch (error) {
      setActionError(actionFailureMessage(error));
    }
  }

  async function openLinkedTask() {
    if (!item.promotedTaskId) return;
    setActionError(undefined);
    try {
      await onOpenTask(item.promotedTaskId);
    } catch {
      setActionError(copy.gmailInflow.linkedTaskProblem);
    }
  }

  return (
    <article
      className="gmail-inflow__detail"
      aria-labelledby={`gmail-inflow-detail-${item.id}`}
    >
      <header>
        <span>{copy.gmailInflow.selectedTitle}</span>
        <h3 id={`gmail-inflow-detail-${item.id}`}>{subjectLabel(item)}</h3>
        <div className="gmail-inflow__source-meta">
          <span>
            <UserRound aria-hidden="true" />
            {senderLabel(item)}
          </span>
          <span>
            <Clock3 aria-hidden="true" />
            {formatReceivedAt(item.receivedAt)}
          </span>
          <WorkspaceBadge item={item} />
        </div>
      </header>

      {item.status === "deferred" && item.deferredUntil && (
        <p className="gmail-inflow__returned">
          {copy.gmailInflow.deferredReturned(
            formatReceivedAt(item.deferredUntil),
          )}
        </p>
      )}

      {item.analysisStatus === "failed" && (
        <div
          className="gmail-inflow__analysis-state"
          data-failed="true"
          role="alert"
        >
          <span>
            {copy.gmailInflow.analysisFailed}
            {item.analysisErrorCode && (
              <small>{copy.gmailInflow.analysisDiagnostic}</small>
            )}
          </span>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => void retryAnalysis()}
          >
            {copy.gmailInflow.retryAnalysis}
          </button>
        </div>
      )}

      {item.promotedTaskId && (
        <div className="gmail-inflow__analysis-state" role="status">
          <span>
            <strong>{copy.gmailInflow.linkedTaskReplyTitle}</strong>
            <small>{copy.gmailInflow.linkedTaskReplyDescription}</small>
          </span>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => void openLinkedTask()}
          >
            {copy.gmailInflow.openLinkedTask}
          </button>
        </div>
      )}

      <form className="gmail-inflow__form" onSubmit={submit}>
        {!item.promotedTaskId && (
          <>
            <label>
              <span>{copy.gmailInflow.project}</span>
              <select
                value={projectId}
                onChange={(event) => setProjectId(event.target.value)}
                disabled={saving}
                required
              >
                <option value="">{copy.gmailInflow.projectPlaceholder}</option>
                {projects.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.title}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>{copy.gmailInflow.suggestedTitle}</span>
              <input
                value={title}
                onChange={(event) => changeDraft("title", event.target.value)}
                disabled={saving}
              />
            </label>
            <label>
              <span>{copy.gmailInflow.suggestedNotes}</span>
              <textarea
                value={notes}
                onChange={(event) => changeDraft("notes", event.target.value)}
                disabled={saving}
                rows={4}
              />
            </label>
            <div className="gmail-inflow__field-grid">
              <label>
                <span>{copy.gmailInflow.assignee}</span>
                <input
                  value={assigneeName}
                  placeholder={copy.gmailInflow.noAssignee}
                  onChange={(event) =>
                    changeDraft("assigneeName", event.target.value)
                  }
                  disabled={saving}
                />
              </label>
              <label>
                <span>{copy.gmailInflow.priority}</span>
                <select
                  value={priority}
                  onChange={(event) =>
                    changeDraft("priority", Number(event.target.value))
                  }
                  disabled={saving}
                >
                  <option value={0}>{copy.forms.priorityNormal}</option>
                  <option value={1}>{copy.forms.prioritySoon}</option>
                  <option value={2}>{copy.forms.priorityImportant}</option>
                  <option value={3}>{copy.forms.priorityHighest}</option>
                </select>
              </label>
            </div>
            <DeadlinePicker
              className="gmail-inflow__due-at"
              id={`gmail-inflow-due-at-${item.id}`}
              label={copy.gmailInflow.suggestedDueAt}
              value={dueAt}
              disabled={saving}
              showPresets
              onChange={(value) => changeDraft("dueAt", value)}
            />
          </>
        )}

        <details className="gmail-inflow__original">
          <summary>{copy.gmailInflow.original}</summary>
          {!item.bodyText && (
            <p className="gmail-inflow__body-unavailable">
              {copy.gmailInflow.bodyUnavailable}
            </p>
          )}
          <p>
            <LinkifiedText text={item.bodyText || item.snippet} />
          </p>
          {item.originalThreadUrl && (
            <SafeExternalLink
              className="gmail-inflow__original-link"
              href={item.originalThreadUrl}
            >
              {copy.gmailInflow.openOriginal}
              <ExternalLink aria-hidden="true" />
            </SafeExternalLink>
          )}
          {item.referenceLinks.length > 0 && (
            <div className="gmail-inflow__references">
              <strong>
                <Link2 aria-hidden="true" />
                {copy.gmailInflow.references}
              </strong>
              <ul>
                {item.referenceLinks.map((link) => (
                  <li key={link}>
                    <SafeExternalLink href={link}>
                      <span>{readableLink(link)}</span>
                      <ExternalLink aria-hidden="true" />
                    </SafeExternalLink>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </details>

        {actionError && (
          <p className="inline-alert" role="alert">
            {actionError}
          </p>
        )}

        <DeadlinePicker
          className="gmail-inflow__defer-at"
          id={`gmail-inflow-defer-at-${item.id}`}
          label={copy.gmailInflow.deferAt}
          value={revisitAt}
          disabled={saving}
          showPresets
          allowClear={false}
          onChange={setRevisitAt}
        />

        <div className="gmail-inflow__actions">
          {!item.promotedTaskId && (
            <button
              className="primary-button focus-visible-control"
              type="submit"
              disabled={saving || !analysisReady}
            >
              {saving ? copy.gmailInflow.promoting : copy.gmailInflow.promote}
            </button>
          )}
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => void decide("defer")}
          >
            {copy.gmailInflow.defer}
          </button>
          <button
            className="text-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => void decide("dismiss")}
          >
            {copy.gmailInflow.dismiss}
          </button>
        </div>
      </form>
    </article>
  );
}

function WorkspaceBadge({ item }: { item: GmailInflowCandidate }) {
  const label =
    item.workspaceScope === "company"
      ? copy.gmailInflow.company
      : copy.gmailInflow.personal;
  const Icon = item.workspaceScope === "company" ? Building2 : UserRound;
  return (
    <span
      className="gmail-inflow__workspace-badge"
      data-scope={item.workspaceScope}
    >
      <Icon aria-hidden="true" />
      {item.workspaceName || label}
    </span>
  );
}

function senderLabel(item: GmailInflowCandidate): string {
  return (
    item.senderName?.trim() ||
    item.senderEmail.trim() ||
    copy.gmailInflow.senderUnknown
  );
}

function subjectLabel(item: GmailInflowCandidate): string {
  return item.subject.trim() || copy.gmailInflow.subjectUnknown;
}

function formatReceivedAt(value: string, compact = false): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return copy.gmailInflow.receivedAtUnknown;
  return new Intl.DateTimeFormat("ko-KR", {
    ...(compact ? {} : { month: "numeric", day: "numeric" }),
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function toLocalDateTime(value: string | null): string {
  return isoToSeoulLocalDateTime(value);
}

function suggestedGmailInflowDraft(
  item: GmailInflowCandidate,
): GmailInflowDraftValues {
  return {
    title: item.suggestedTaskTitle,
    notes: item.suggestedTaskNotes,
    assigneeName: item.suggestedAssigneeName ?? "",
    priority: item.suggestedPriority ?? 1,
    dueAt: toLocalDateTime(item.suggestedDueAt),
  };
}

export function mergeGmailInflowDraftValues(
  current: GmailInflowDraftValues,
  suggested: GmailInflowDraftValues,
  dirtyFields: Iterable<GmailInflowDraftField>,
): GmailInflowDraftValues {
  const dirty = new Set(dirtyFields);
  return {
    title: dirty.has("title") ? current.title : suggested.title,
    notes: dirty.has("notes") ? current.notes : suggested.notes,
    assigneeName: dirty.has("assigneeName")
      ? current.assigneeName
      : suggested.assigneeName,
    priority: dirty.has("priority") ? current.priority : suggested.priority,
    dueAt: dirty.has("dueAt") ? current.dueAt : suggested.dueAt,
  };
}

export function localDateTimeToIso(value: string): string | null {
  return seoulLocalDateTimeToIso(value) ?? null;
}

export function deferDateTimeToIso(
  value: string,
  now = new Date(),
): string | null {
  const revisitAt = localDateTimeToIso(value);
  if (!revisitAt) return null;
  const revisitAtMillis = new Date(revisitAt).getTime();
  const nowMillis = now.getTime();
  return revisitAtMillis > nowMillis &&
    revisitAtMillis <= nowMillis + 365 * 24 * 60 * 60 * 1_000
    ? revisitAt
    : null;
}

function defaultRevisitAtLocal(): string {
  return toLocalDateTime(
    new Date(Date.now() + 4 * 60 * 60 * 1_000).toISOString(),
  );
}

function readableLink(value: string): string {
  try {
    const url = new URL(value);
    return `${url.hostname}${url.pathname === "/" ? "" : url.pathname}`;
  } catch {
    return value;
  }
}

export function actionFailureMessage(error: unknown): string {
  return error instanceof PlanningRequestError && error.code === "conflict"
    ? copy.gmailInflow.decisionConflict
    : copy.gmailInflow.decisionProblem;
}
