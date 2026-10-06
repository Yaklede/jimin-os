import { ChevronDown, MessageCircleMore } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { type ProjectInflowItem } from "../api/googleChat";
import { copy } from "../copy";
import {
  InflowItemRow,
  InflowItemList,
  inflowConversationKey,
  type PromoteInflowInput,
} from "./ProjectInflowPanel";

type HomeInflowReviewProps = {
  items: ProjectInflowItem[];
  saving: boolean;
  onPromote(item: ProjectInflowItem, input: PromoteInflowInput): Promise<void>;
  onDismiss(
    item: ProjectInflowItem,
    input?: { reason?: string; replyToSource?: boolean; markSeen?: boolean },
  ): Promise<void>;
  onRetryAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryCompletion(item: ProjectInflowItem): Promise<void>;
  onOpenTask(taskId: string): Promise<void>;
};

export function HomeInflowReview({ ...props }: HomeInflowReviewProps) {
  const pending = homeInflowPendingItems(props.items);
  const unread = pending.filter((item) => !item.reviewed);
  const reviewed = pending.filter((item) => item.reviewed);
  const groups = {
    new: unread.filter((item) => !item.promotedTaskId),
    existing: unread.filter((item) => item.promotedTaskId),
  };
  const [activeTab, setActiveTab] = useState<"new" | "existing">(() =>
    groups.new.length === 0 && groups.existing.length > 0 ? "existing" : "new",
  );
  if (pending.length === 0) return null;
  return (
    <div className="inflow-review-queues">
      <div
        className="home-inflow-tabs"
        role="tablist"
        aria-label={copy.projects.inflowTabsLabel}
      >
        {(["new", "existing"] as const).map((kind, index) => (
          <button
            key={kind}
            id={`home-inflow-tab-${kind}`}
            className="focus-visible-control"
            type="button"
            role="tab"
            aria-selected={activeTab === kind}
            aria-controls={`home-inflow-panel-${kind}`}
            tabIndex={activeTab === kind ? 0 : -1}
            onClick={() => setActiveTab(kind)}
            onKeyDown={(event) => {
              const next =
                event.key === "Home"
                  ? 0
                  : event.key === "End"
                    ? 1
                    : event.key === "ArrowRight" || event.key === "ArrowLeft"
                      ? 1 - index
                      : null;
              if (next === null) return;
              event.preventDefault();
              setActiveTab(next === 0 ? "new" : "existing");
              event.currentTarget.parentElement
                ?.querySelectorAll<HTMLButtonElement>('[role="tab"]')
                [next]?.focus();
            }}
          >
            <span className="home-inflow-tabs__full-label">
              {kind === "new"
                ? copy.projects.inflowNewTitle
                : copy.projects.inflowExistingTitle}
            </span>
            <span className="home-inflow-tabs__short-label">
              {kind === "new"
                ? copy.projects.inflowNewTabShort
                : copy.projects.inflowExistingTabShort}
            </span>
            <span className="home-inflow-tabs__count">
              {groups[kind].length}
            </span>
          </button>
        ))}
      </div>
      {(["new", "existing"] as const).map((kind) => (
        <div
          key={kind}
          className="home-inflow-tab-panel"
          id={`home-inflow-panel-${kind}`}
          role="tabpanel"
          aria-labelledby={`home-inflow-tab-${kind}`}
          hidden={activeTab !== kind}
          tabIndex={0}
        >
          {groups[kind].length > 0 ? (
            <HomeInflowGroup {...props} items={groups[kind]} kind={kind} />
          ) : (
            <p className="home-inflow-tabs__empty">
              {kind === "new"
                ? copy.projects.inflowNewEmpty
                : copy.projects.inflowExistingEmpty}
            </p>
          )}
        </div>
      ))}
      {reviewed.length > 0 && (
        <details className="project-inflow__history">
          <summary className="focus-visible-control">
            {copy.projects.inflowReviewedTitle} · {reviewed.length}개
          </summary>
          <InflowItemList
            {...props}
            items={reviewed}
            title={copy.projects.inflowReviewedTitle}
            onOpenTask={(taskId) => void props.onOpenTask(taskId)}
          />
        </details>
      )}
    </div>
  );
}

function HomeInflowGroup({
  items,
  saving,
  onPromote,
  onDismiss,
  onRetryAnalysis,
  onRetryCompletion,
  onOpenTask,
  kind,
}: HomeInflowReviewProps & { kind: "new" | "existing" }) {
  const titleId = `home-inflow-title-${kind}`;
  const detailId = `home-inflow-detail-title-${kind}`;
  const pendingItems = useMemo(() => homeInflowPendingItems(items), [items]);
  const [showAll, setShowAll] = useState(false);
  const visibleItems = useMemo(
    () => visibleHomeInflowItems(pendingItems, showAll),
    [pendingItems, showAll],
  );
  const [selectedConversationId, setSelectedConversationId] = useState(
    visibleItems[0] ? inflowConversationKey(visibleItems[0]) : undefined,
  );
  const [mobileExpanded, setMobileExpanded] = useState(false);
  const selectedItem = resolveHomeInflowSelection(
    visibleItems,
    selectedConversationId,
  );

  useEffect(() => {
    const nextSelection = selectedItem
      ? inflowConversationKey(selectedItem)
      : undefined;
    if (selectedConversationId !== nextSelection) {
      setSelectedConversationId(nextSelection);
    }
  }, [selectedConversationId, selectedItem]);

  if (!selectedItem) return null;

  return (
    <section
      className="home-inflow"
      aria-labelledby={titleId}
      data-mobile-expanded={mobileExpanded}
    >
      <header className="home-inflow__heading">
        <div className="home-inflow__heading-copy">
          <span>
            {kind === "new"
              ? copy.projects.inflowHomeEyebrow
              : copy.projects.inflowExistingEyebrow}
          </span>
          <h2 id={titleId}>
            {kind === "new"
              ? copy.projects.inflowNewTitle
              : copy.projects.inflowExistingTitle}
          </h2>
          <p>
            {kind === "new"
              ? copy.projects.inflowHomeDescription
              : copy.projects.inflowExistingDescription}
          </p>
        </div>
        <strong aria-label={`${pendingItems.length}개의 업무 요청`}>
          {pendingItems.length}
        </strong>
      </header>

      <button
        className="home-inflow__mobile-toggle focus-visible-control"
        type="button"
        aria-expanded={mobileExpanded}
        onClick={() => setMobileExpanded((current) => !current)}
      >
        <MessageCircleMore aria-hidden="true" />
        <span>
          {mobileExpanded
            ? kind === "new"
              ? copy.projects.inflowHomeCollapse
              : copy.projects.inflowExistingCollapse
            : kind === "new"
              ? copy.projects.inflowHomeOpen(pendingItems.length)
              : copy.projects.inflowExistingOpen(pendingItems.length)}
        </span>
        <ChevronDown aria-hidden="true" />
      </button>

      <div className="home-inflow-review">
        <aside
          className="home-inflow-review__queue"
          aria-labelledby={`home-inflow-queue-title-${kind}`}
        >
          <div className="home-inflow-review__queue-heading">
            <MessageCircleMore aria-hidden="true" />
            <strong id={`home-inflow-queue-title-${kind}`}>
              {kind === "new"
                ? copy.projects.inflowHomeQueueTitle
                : copy.projects.inflowExistingQueueTitle}
            </strong>
            <span>{visibleItems.length}</span>
          </div>
          <ol>
            {visibleItems.map((item) => {
              const conversationId = inflowConversationKey(item);
              const active =
                conversationId === inflowConversationKey(selectedItem);
              return (
                <li key={conversationId}>
                  <button
                    className="home-inflow-review__queue-item focus-visible-control"
                    type="button"
                    aria-pressed={active}
                    data-active={active}
                    onClick={() => setSelectedConversationId(conversationId)}
                  >
                    <span className="home-inflow-review__queue-meta">
                      <strong>
                        {item.senderName ?? copy.projects.inflowSenderPending}
                      </strong>
                      <time dateTime={item.receivedAt}>
                        {formatHomeInflowTime(item.receivedAt)}
                      </time>
                    </span>
                    <span className="home-inflow-review__queue-title">
                      {item.suggestedTaskTitle}
                    </span>
                    <small>{item.sourceName}</small>
                  </button>
                </li>
              );
            })}
          </ol>
          {pendingItems.length > 5 && (
            <button
              className="home-inflow-review__show-all focus-visible-control"
              type="button"
              aria-expanded={showAll}
              onClick={() => setShowAll((current) => !current)}
            >
              <span>
                {showAll
                  ? copy.projects.inflowHomeShowLess
                  : copy.projects.inflowHomeShowAll(pendingItems.length)}
              </span>
              <ChevronDown aria-hidden="true" />
            </button>
          )}
        </aside>

        <section
          className="home-inflow-review__detail"
          aria-labelledby={detailId}
        >
          <header>
            <span>{copy.projects.inflowHomeSelectedLabel}</span>
            <strong id={detailId}>
              {copy.projects.inflowHomeSelectedRequest(
                selectedItem.senderName || "",
              )}
            </strong>
          </header>
          <ul>
            <InflowItemRow
              key={inflowConversationKey(selectedItem)}
              item={selectedItem}
              saving={saving}
              onPromote={onPromote}
              onDismiss={onDismiss}
              onRetryAnalysis={onRetryAnalysis}
              onRetryCompletion={onRetryCompletion}
              onOpenTask={(taskId) => void onOpenTask(taskId)}
            />
          </ul>
        </section>
      </div>
    </section>
  );
}

export function homeInflowPendingItems(
  items: ProjectInflowItem[],
): ProjectInflowItem[] {
  return items.filter((item) => item.status === "pending");
}

export function visibleHomeInflowItems(
  items: ProjectInflowItem[],
  showAll: boolean,
): ProjectInflowItem[] {
  return showAll ? items : items.slice(0, 5);
}

export function resolveHomeInflowSelection(
  items: ProjectInflowItem[],
  selectedConversationId: string | undefined,
): ProjectInflowItem | undefined {
  return (
    items.find(
      (item) => inflowConversationKey(item) === selectedConversationId,
    ) ?? items[0]
  );
}

function formatHomeInflowTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "받은 시간 확인 필요";
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
