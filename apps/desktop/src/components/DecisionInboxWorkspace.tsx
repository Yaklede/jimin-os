import {
  CheckCircle2,
  ChevronDown,
  Clock3,
  Inbox,
  ShieldAlert,
  XCircle,
} from "lucide-react";
import { useMemo, useState } from "react";

import { type Recommendation } from "../api/home";
import {
  projectInflowPromotionReadiness,
  type ProjectInflowItem,
} from "../api/googleChat";
import { type RecommendationDecision } from "../api/intelligence";
import { type ProjectItsmDecisionCandidate } from "../api/itsm";
import { copy } from "../copy";
import { EmptySurface } from "./HomeWorkspace";
import {
  GmailInflowReview,
  type GmailInflowReviewProps,
} from "./GmailInflowReview";
import {
  InflowItemRow,
  inflowConversationKey,
  type PromoteInflowInput,
} from "./ProjectInflowPanel";

type DecisionInboxWorkspaceProps = {
  recommendations: Recommendation[];
  inflowItems: ProjectInflowItem[];
  itsmCandidates: ProjectItsmDecisionCandidate[];
  loading: boolean;
  error: string | undefined;
  inflowSaving: boolean;
  gmailReview: GmailInflowReviewProps;
  onOpenConversation(conversationId: string): void;
  onOpenTask(taskId: string): Promise<void>;
  onPromoteInflow(
    item: ProjectInflowItem,
    input: PromoteInflowInput,
  ): Promise<void>;
  onDismissInflow(item: ProjectInflowItem): Promise<void>;
  onRetryInflowAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryInflowCompletion(item: ProjectInflowItem): Promise<void>;
  onConfirmItsm(candidate: ProjectItsmDecisionCandidate): Promise<void>;
  onDecide(
    recommendation: Recommendation,
    decision: RecommendationDecision,
  ): Promise<boolean>;
  onRetryAnalysis(recommendation: Recommendation): Promise<boolean>;
};

export function DecisionInboxWorkspace({
  recommendations,
  inflowItems,
  itsmCandidates,
  loading,
  error,
  inflowSaving,
  gmailReview,
  onOpenConversation,
  onOpenTask,
  onPromoteInflow,
  onDismissInflow,
  onRetryInflowAnalysis,
  onRetryInflowCompletion,
  onConfirmItsm,
  onDecide,
  onRetryAnalysis,
}: DecisionInboxWorkspaceProps) {
  const [pendingId, setPendingId] = useState<string>();
  const [confirmingItsmId, setConfirmingItsmId] = useState<string>();
  const [decisionError, setDecisionError] = useState<string>();
  const pending = useMemo(
    () => recommendations.filter((item) => isDecisionActionableNow(item)),
    [recommendations],
  );
  const inProgress = useMemo(
    () => recommendations.filter((item) => isDecisionInProgress(item)),
    [recommendations],
  );
  const retryable = useMemo(
    () => recommendations.filter((item) => item.status === "failed"),
    [recommendations],
  );
  const history = useMemo(
    () =>
      recommendations.filter(
        (item) =>
          !isDecisionActionableNow(item) &&
          !isDecisionInProgress(item) &&
          item.status !== "failed",
      ),
    [recommendations],
  );
  const pendingInflow = useMemo(
    () => inflowItems.filter(isProjectInflowDecisionItem),
    [inflowItems],
  );

  async function decide(
    recommendation: Recommendation,
    decision: RecommendationDecision,
  ) {
    if (pendingId) return;
    setPendingId(recommendation.id);
    setDecisionError(undefined);
    const succeeded = await onDecide(recommendation, decision);
    setPendingId(undefined);
    if (!succeeded) {
      setDecisionError(copy.decisions.decisionNotice);
    }
  }

  async function confirmItsm(candidate: ProjectItsmDecisionCandidate) {
    if (confirmingItsmId) return;
    setConfirmingItsmId(candidate.connection.id);
    setDecisionError(undefined);
    try {
      await onConfirmItsm(candidate);
    } catch {
      setDecisionError(copy.decisions.confirmItsmProblem);
    } finally {
      setConfirmingItsmId(undefined);
    }
  }

  async function retryAnalysis(recommendation: Recommendation) {
    if (pendingId) return;
    setPendingId(recommendation.id);
    setDecisionError(undefined);
    const succeeded = await onRetryAnalysis(recommendation);
    setPendingId(undefined);
    if (!succeeded) {
      setDecisionError(copy.decisions.retryAnalysisProblem);
    }
  }

  return (
    <section className="decision-page" aria-busy={loading}>
      <header className="page-heading decision-page__heading">
        <div>
          <span>{copy.decisions.eyebrow}</span>
          <h1>{copy.decisions.title}</h1>
          <p>{copy.decisions.description}</p>
        </div>
        <span className="decision-page__symbol" aria-hidden="true">
          <Inbox />
        </span>
      </header>

      {(error || decisionError) && (
        <p className="inline-alert" role="alert">
          {decisionError ?? error}
        </p>
      )}

      {loading &&
      recommendations.length === 0 &&
      pendingInflow.length === 0 &&
      itsmCandidates.length === 0 &&
      gmailReview.items.length === 0 ? (
        <DecisionInboxSkeleton />
      ) : (
        <>
          <ItsmConfirmationSection
            items={itsmCandidates}
            confirmingId={confirmingItsmId}
            onConfirm={confirmItsm}
          />
          <InflowDecisionSection
            items={pendingInflow}
            saving={inflowSaving}
            onPromote={onPromoteInflow}
            onDismiss={onDismissInflow}
            onRetryAnalysis={onRetryInflowAnalysis}
            onRetryCompletion={onRetryInflowCompletion}
            onOpenTask={onOpenTask}
          />
          <GmailInflowReview {...gmailReview} />
          <DecisionSection
            id="pending-decisions"
            title={copy.decisions.pendingTitle}
            items={pending}
            pendingId={pendingId}
            emptyTitle={copy.decisions.emptyPendingTitle}
            emptyDescription={copy.decisions.emptyPendingDescription}
            onOpenConversation={onOpenConversation}
            onDecide={decide}
            onRetryAnalysis={retryAnalysis}
          />
          {inProgress.length > 0 && (
            <DecisionSection
              id="in-progress-decisions"
              title={copy.decisions.inProgressTitle}
              items={inProgress}
              pendingId={pendingId}
              emptyTitle=""
              emptyDescription=""
              onOpenConversation={onOpenConversation}
              onDecide={decide}
              onRetryAnalysis={retryAnalysis}
            />
          )}
          {retryable.length > 0 && (
            <DecisionSection
              id="retry-decisions"
              title={copy.decisions.retryTitle}
              items={retryable}
              pendingId={pendingId}
              emptyTitle=""
              emptyDescription=""
              onOpenConversation={onOpenConversation}
              onDecide={decide}
              onRetryAnalysis={retryAnalysis}
            />
          )}
          <DecisionHistory
            items={history}
            pendingId={pendingId}
            onOpenConversation={onOpenConversation}
            onDecide={decide}
            onRetryAnalysis={retryAnalysis}
          />
        </>
      )}
    </section>
  );
}

function ItsmConfirmationSection({
  items,
  confirmingId,
  onConfirm,
}: {
  items: ProjectItsmDecisionCandidate[];
  confirmingId: string | undefined;
  onConfirm(candidate: ProjectItsmDecisionCandidate): Promise<void>;
}) {
  if (items.length === 0) return null;
  return (
    <section className="decision-section" aria-labelledby="itsm-decisions">
      <header>
        <h2 id="itsm-decisions">{copy.decisions.itsmTitle}</h2>
        <span>{copy.decisions.count(items.length)}</span>
      </header>
      <ol>
        {items.map((candidate) => (
          <li
            className="decision-card decision-card--itsm"
            data-status="pending"
            key={candidate.connection.id}
          >
            <span className="decision-card__icon" aria-hidden="true">
              <ShieldAlert />
            </span>
            <div className="decision-card__body">
              <div className="decision-card__title-row">
                <h3>
                  {copy.decisions.itsmCandidateTitle(
                    candidate.connection.candidateProjectName ??
                      candidate.projectName,
                  )}
                </h3>
                <span>{copy.decisions.inflowStatus}</span>
              </div>
              <p>{copy.decisions.itsmCandidateDescription}</p>
              <dl>
                <div>
                  <dt>{copy.decisions.project}</dt>
                  <dd>{candidate.projectName}</dd>
                </div>
              </dl>
            </div>
            <div className="decision-card__actions">
              <button
                className="primary-button focus-visible-control"
                type="button"
                disabled={Boolean(confirmingId)}
                onClick={() => void onConfirm(candidate)}
              >
                {confirmingId === candidate.connection.id && (
                  <span className="button-spinner" aria-hidden="true" />
                )}
                {copy.decisions.confirmItsm}
              </button>
            </div>
          </li>
        ))}
      </ol>
    </section>
  );
}

function InflowDecisionSection({
  items,
  saving,
  onPromote,
  onDismiss,
  onRetryAnalysis,
  onRetryCompletion,
  onOpenTask,
}: {
  items: ProjectInflowItem[];
  saving: boolean;
  onPromote(item: ProjectInflowItem, input: PromoteInflowInput): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
  onRetryAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryCompletion(item: ProjectInflowItem): Promise<void>;
  onOpenTask(taskId: string): Promise<void>;
}) {
  return (
    <section className="decision-section" aria-labelledby="inflow-decisions">
      <header>
        <h2 id="inflow-decisions">{copy.decisions.inflowTitle}</h2>
        <span>{copy.decisions.count(items.length)}</span>
      </header>
      {items.length === 0 ? (
        <EmptySurface
          title={copy.decisions.emptyInflowTitle}
          description={copy.decisions.emptyInflowDescription}
        />
      ) : (
        <ul className="decision-section__inflow-list">
          {items.map((item) => (
            <InflowItemRow
              key={inflowConversationKey(item)}
              item={item}
              saving={saving}
              onPromote={onPromote}
              onDismiss={onDismiss}
              onRetryAnalysis={onRetryAnalysis}
              onRetryCompletion={onRetryCompletion}
              onOpenTask={(taskId) => void onOpenTask(taskId)}
            />
          ))}
        </ul>
      )}
    </section>
  );
}

export function inflowDecisionSummary(item: ProjectInflowItem): string {
  const decisions: string[] = [copy.decisions.promoteDecision];
  if (!item.suggestedAssigneeName) decisions.push(copy.decisions.assignee);
  if (!item.suggestedDueAt) decisions.push(copy.decisions.deadline);
  return decisions.join(" · ");
}

export function isProjectInflowDecisionItem(item: ProjectInflowItem): boolean {
  if (item.status === "promoted") return item.completionStatus === "failed";
  if (item.status !== "pending") return false;
  if (item.promotedTaskId) return true;
  if (item.analysisStatus === "failed" || item.analysisStatus === "stale") {
    return true;
  }
  return projectInflowPromotionReadiness(item).canPromote;
}

function DecisionSection({
  id,
  title,
  items,
  pendingId,
  emptyTitle,
  emptyDescription,
  onOpenConversation,
  onDecide,
  onRetryAnalysis,
}: {
  id: string;
  title: string;
  items: Recommendation[];
  pendingId: string | undefined;
  emptyTitle: string;
  emptyDescription: string;
  onOpenConversation(conversationId: string): void;
  onDecide(
    recommendation: Recommendation,
    decision: RecommendationDecision,
  ): Promise<void>;
  onRetryAnalysis(recommendation: Recommendation): Promise<void>;
}) {
  return (
    <section className="decision-section" aria-labelledby={id}>
      <header>
        <h2 id={id}>{title}</h2>
        <span>{copy.decisions.count(items.length)}</span>
      </header>
      {items.length === 0 ? (
        <EmptySurface title={emptyTitle} description={emptyDescription} />
      ) : (
        <ol>
          {items.map((recommendation) => (
            <DecisionCard
              key={recommendation.id}
              recommendation={recommendation}
              pending={pendingId === recommendation.id}
              interactionLocked={Boolean(pendingId)}
              onOpenConversation={onOpenConversation}
              onDecide={onDecide}
              onRetryAnalysis={onRetryAnalysis}
            />
          ))}
        </ol>
      )}
    </section>
  );
}

function DecisionHistory({
  items,
  pendingId,
  onOpenConversation,
  onDecide,
  onRetryAnalysis,
}: {
  items: Recommendation[];
  pendingId: string | undefined;
  onOpenConversation(conversationId: string): void;
  onDecide(
    recommendation: Recommendation,
    decision: RecommendationDecision,
  ): Promise<void>;
  onRetryAnalysis(recommendation: Recommendation): Promise<void>;
}) {
  if (items.length === 0) return null;
  return (
    <details className="decision-history">
      <summary className="focus-visible-control">
        <span>{copy.decisions.historyTitle}</span>
        <small>{copy.decisions.count(items.length)}</small>
        <ChevronDown aria-hidden="true" />
      </summary>
      <ol>
        {items.map((recommendation) => (
          <DecisionCard
            key={recommendation.id}
            recommendation={recommendation}
            pending={pendingId === recommendation.id}
            interactionLocked={Boolean(pendingId)}
            onOpenConversation={onOpenConversation}
            onDecide={onDecide}
            onRetryAnalysis={onRetryAnalysis}
          />
        ))}
      </ol>
    </details>
  );
}

function DecisionCard({
  recommendation,
  pending,
  interactionLocked,
  onOpenConversation,
  onDecide,
  onRetryAnalysis,
}: {
  recommendation: Recommendation;
  pending: boolean;
  interactionLocked: boolean;
  onOpenConversation(conversationId: string): void;
  onDecide(
    recommendation: Recommendation,
    decision: RecommendationDecision,
  ): Promise<void>;
  onRetryAnalysis(recommendation: Recommendation): Promise<void>;
}) {
  const actionable = isDecisionActionableNow(recommendation);
  const retryable = recommendation.status === "failed";
  const conversationDecision = isConversationDecision(recommendation);
  return (
    <li className="decision-card" data-status={recommendation.status}>
      <span className="decision-card__icon" aria-hidden="true">
        <StatusIcon status={recommendation.status} />
      </span>
      <div className="decision-card__body">
        <div className="decision-card__title-row">
          <h3>{recommendation.title}</h3>
          <span>{statusLabel(recommendation.status)}</span>
        </div>
        <p>{recommendation.rationale}</p>
        <dl>
          <div>
            <dt>{copy.decisions.expectedEffect}</dt>
            <dd>{recommendation.expectedEffect}</dd>
          </div>
          {recommendation.riskSummary && (
            <div>
              <dt>{copy.decisions.risk}</dt>
              <dd>{recommendation.riskSummary}</dd>
            </div>
          )}
        </dl>
        <time dateTime={recommendation.updatedAt}>
          {formatDecisionTime(recommendation.updatedAt)}
        </time>
      </div>
      {actionable && (
        <div className="decision-card__actions">
          <button
            className="text-button focus-visible-control"
            type="button"
            disabled={interactionLocked}
            onClick={() => void onDecide(recommendation, "reject")}
          >
            {copy.decisions.reject}
          </button>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={interactionLocked}
            onClick={() => void onDecide(recommendation, "defer")}
          >
            {copy.decisions.defer}
          </button>
          <button
            className="primary-button focus-visible-control"
            type="button"
            disabled={interactionLocked}
            onClick={() => {
              if (conversationDecision && recommendation.suggestedEntityId) {
                onOpenConversation(recommendation.suggestedEntityId);
                return;
              }
              void onDecide(recommendation, "approve");
            }}
          >
            {pending && <span className="button-spinner" aria-hidden="true" />}
            {conversationDecision
              ? copy.decisions.openConversation
              : copy.decisions.approve}
          </button>
        </div>
      )}
      {retryable && (
        <div className="decision-card__actions">
          <button
            className="primary-button focus-visible-control"
            type="button"
            disabled={interactionLocked}
            onClick={() => void onRetryAnalysis(recommendation)}
          >
            {pending && <span className="button-spinner" aria-hidden="true" />}
            {copy.decisions.retryAnalysis}
          </button>
        </div>
      )}
    </li>
  );
}

export function isDecisionActionableNow(
  recommendation: Recommendation,
  now = Date.now(),
): boolean {
  if (
    recommendation.status === "pending" ||
    recommendation.status === "analysis_requested"
  ) {
    return true;
  }
  if (recommendation.status !== "deferred" || !recommendation.revisitAt) {
    return false;
  }
  const revisitAt = new Date(recommendation.revisitAt).getTime();
  return Number.isFinite(revisitAt) && revisitAt <= now;
}

export function isDecisionInProgress(recommendation: Recommendation): boolean {
  return (
    recommendation.status === "approved" ||
    recommendation.status === "executing"
  );
}

export function isConversationDecision(
  recommendation: Recommendation,
): boolean {
  return (
    recommendation.suggestedActionKind === "request_analysis" &&
    Boolean(recommendation.suggestedEntityId)
  );
}

function StatusIcon({ status }: { status: Recommendation["status"] }) {
  if (status === "executed") return <CheckCircle2 />;
  if (status === "rejected" || status === "expired") return <XCircle />;
  if (status === "failed") return <ShieldAlert />;
  return <Clock3 />;
}

function statusLabel(status: Recommendation["status"]): string {
  return copy.decisions.status[status];
}

function formatDecisionTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat("ko-KR", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function DecisionInboxSkeleton() {
  return (
    <div className="decision-page__skeleton" aria-hidden="true">
      <span />
      <span />
      <span />
    </div>
  );
}
