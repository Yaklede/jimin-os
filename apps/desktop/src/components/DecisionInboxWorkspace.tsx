import {
  ArrowRight,
  CheckCircle2,
  Clock3,
  Inbox,
  MessagesSquare,
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

type DecisionInboxWorkspaceProps = {
  recommendations: Recommendation[];
  inflowItems: ProjectInflowItem[];
  itsmCandidates: ProjectItsmDecisionCandidate[];
  loading: boolean;
  error: string | undefined;
  onOpenConversation(conversationId: string): void;
  onOpenProjectInflow(item: ProjectInflowItem): Promise<void>;
  onConfirmItsm(candidate: ProjectItsmDecisionCandidate): Promise<void>;
  onDecide(
    recommendation: Recommendation,
    decision: RecommendationDecision,
  ): Promise<boolean>;
};

export function DecisionInboxWorkspace({
  recommendations,
  inflowItems,
  itsmCandidates,
  loading,
  error,
  onOpenConversation,
  onOpenProjectInflow,
  onConfirmItsm,
  onDecide,
}: DecisionInboxWorkspaceProps) {
  const [pendingId, setPendingId] = useState<string>();
  const [openingInflowId, setOpeningInflowId] = useState<string>();
  const [confirmingItsmId, setConfirmingItsmId] = useState<string>();
  const [decisionError, setDecisionError] = useState<string>();
  const pending = useMemo(
    () => recommendations.filter((item) => isDecisionActionableNow(item)),
    [recommendations],
  );
  const history = useMemo(
    () => recommendations.filter((item) => !isDecisionActionableNow(item)),
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

  async function openProjectInflow(item: ProjectInflowItem) {
    if (openingInflowId) return;
    setOpeningInflowId(item.id);
    setDecisionError(undefined);
    try {
      await onOpenProjectInflow(item);
    } catch {
      setDecisionError(copy.decisions.openInflowProblem);
    } finally {
      setOpeningInflowId(undefined);
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
      itsmCandidates.length === 0 ? (
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
            openingId={openingInflowId}
            onOpen={openProjectInflow}
          />
          <DecisionSection
            id="pending-decisions"
            title={copy.decisions.pendingTitle}
            items={pending}
            pendingId={pendingId}
            emptyTitle={copy.decisions.emptyPendingTitle}
            emptyDescription={copy.decisions.emptyPendingDescription}
            onOpenConversation={onOpenConversation}
            onDecide={decide}
          />
          <DecisionSection
            id="decision-history"
            title={copy.decisions.historyTitle}
            items={history}
            pendingId={pendingId}
            emptyTitle={copy.decisions.emptyHistoryTitle}
            emptyDescription={copy.decisions.emptyHistoryDescription}
            onOpenConversation={onOpenConversation}
            onDecide={decide}
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
  openingId,
  onOpen,
}: {
  items: ProjectInflowItem[];
  openingId: string | undefined;
  onOpen(item: ProjectInflowItem): Promise<void>;
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
        <ol>
          {items.map((item) => (
            <li
              className="decision-card decision-card--inflow"
              data-status="pending"
              key={item.id}
            >
              <span className="decision-card__icon" aria-hidden="true">
                <MessagesSquare />
              </span>
              <div className="decision-card__body">
                <div className="decision-card__title-row">
                  <h3>{item.suggestedTaskTitle}</h3>
                  <span>{copy.decisions.inflowStatus}</span>
                </div>
                <p>
                  {item.analysisSummary?.trim() ||
                    compactInflowContent(item.contentText)}
                </p>
                <dl>
                  <div>
                    <dt>{copy.decisions.project}</dt>
                    <dd>
                      {item.projectName} · {item.sourceName}
                    </dd>
                  </div>
                  <div>
                    <dt>{copy.decisions.needsDecision}</dt>
                    <dd>{inflowDecisionSummary(item)}</dd>
                  </div>
                </dl>
                <time dateTime={item.receivedAt}>
                  {formatDecisionTime(item.receivedAt)}
                </time>
              </div>
              <div className="decision-card__actions">
                <button
                  className="primary-button focus-visible-control"
                  type="button"
                  disabled={Boolean(openingId)}
                  onClick={() => void onOpen(item)}
                >
                  {openingId === item.id && (
                    <span className="button-spinner" aria-hidden="true" />
                  )}
                  {copy.decisions.openInProject}
                  <ArrowRight aria-hidden="true" />
                </button>
              </div>
            </li>
          ))}
        </ol>
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
  return (
    item.status === "pending" &&
    !item.promotedTaskId &&
    projectInflowPromotionReadiness(item).canPromote
  );
}

function compactInflowContent(value: string): string {
  const compact = value.replace(/\s+/g, " ").trim();
  if (compact.length <= 180) return compact;
  return `${compact.slice(0, 179)}…`;
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
            />
          ))}
        </ol>
      )}
    </section>
  );
}

function DecisionCard({
  recommendation,
  pending,
  interactionLocked,
  onOpenConversation,
  onDecide,
}: {
  recommendation: Recommendation;
  pending: boolean;
  interactionLocked: boolean;
  onOpenConversation(conversationId: string): void;
  onDecide(
    recommendation: Recommendation,
    decision: RecommendationDecision,
  ): Promise<void>;
}) {
  const actionable = isDecisionActionableNow(recommendation);
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
