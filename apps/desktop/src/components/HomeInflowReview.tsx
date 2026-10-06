import { ChevronDown, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { type ProjectInflowItem } from "../api/googleChat";
import { copy } from "../copy";
import {
  homeInflowByReceivedDate,
  homeInflowOnDate,
} from "../home-inflow-dates";
import { planningViewRange } from "../planningRange";
import { HomeInflowCalendarView } from "./home-inflow-calendar-view";
import {
  InflowItemRow,
  InflowItemList,
  inflowConversationKey,
  isProjectInflowAttentionItem,
  type PromoteInflowInput,
} from "./ProjectInflowPanel";

type HomeInflowReviewProps = {
  initialView?: "list" | "calendar";
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
  initialView = "calendar",
}: HomeInflowReviewProps & { kind: "new" | "existing" }) {
  const titleId = `home-inflow-title-${kind}`;
  const detailId = `home-inflow-detail-title-${kind}`;
  const queueTitleId = `home-inflow-queue-title-${kind}`;
  const reviewRef = useRef<HTMLDivElement>(null);
  const queueRef = useRef<HTMLOListElement>(null);
  const [queueOverflows, setQueueOverflows] = useState(false);
  const allItems = useMemo(
    () => homeInflowByReceivedDate(items.filter(isProjectInflowAttentionItem)),
    [items],
  );
  const [view, setView] = useState<"list" | "calendar">(initialView);
  const [range, setRange] = useState(() => planningViewRange("month"));
  const [calendarDetailOpen, setCalendarDetailOpen] = useState(false);
  const visibleItems = useMemo(
    () =>
      view === "calendar" ? homeInflowOnDate(allItems, range.anchor) : allItems,
    [allItems, view, range.anchor],
  );
  const showDetail = view === "list" || calendarDetailOpen;

  useEffect(() => {
    const queue = queueRef.current;
    if (!queue || !showDetail) return;
    queue.scrollTop = 0;
    const measure = () => {
      setQueueOverflows(queue.scrollHeight > queue.clientHeight + 1);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(queue);
    for (const row of queue.children) observer.observe(row);
    return () => observer.disconnect();
  }, [visibleItems, showDetail]);
  const [selectedConversationId, setSelectedConversationId] = useState(
    visibleItems[0] ? inflowConversationKey(visibleItems[0]) : undefined,
  );

  const selectedItem =
    visibleItems.find(
      (item) => inflowConversationKey(item) === selectedConversationId,
    ) ?? visibleItems[0];

  if (!allItems.length) return null;
  const dateLabel = new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    weekday: "short",
  }).format(range.anchor);

  return (
    <section
      className="home-inflow"
      aria-labelledby={titleId}
      data-view={view}
      data-detail-open={showDetail}
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
        <strong
          aria-label={copy.projects.inflowHomeRequestCount(allItems.length)}
        >
          {allItems.length}
        </strong>
      </header>

      <div
        className="home-inflow__view-controls"
        role="group"
        aria-label={copy.projects.inflowHomeViews}
      >
        {(["list", "calendar"] as const).map((mode) => (
          <button
            type="button"
            key={mode}
            className="focus-visible-control"
            aria-pressed={view === mode}
            data-active={view === mode}
            onClick={() => setView(mode)}
          >
            {mode === "list"
              ? copy.projects.inflowHomeListView
              : copy.projects.inflowHomeCalendarView}
          </button>
        ))}
      </div>
      <div className="home-inflow-review" ref={reviewRef}>
        {view === "calendar" && (
          <HomeInflowCalendarView
            kind={kind}
            range={range}
            items={allItems}
            onRangeChange={(next) => {
              setRange(next);
              setSelectedConversationId(undefined);
            }}
            onSelectItem={(item) => {
              setRange(
                planningViewRange(range.mode, new Date(item.receivedAt)),
              );
              setSelectedConversationId(inflowConversationKey(item));
              setCalendarDetailOpen(true);
            }}
            onSelectDate={(date) => {
              setRange(planningViewRange(range.mode, date));
              setSelectedConversationId(undefined);
              setCalendarDetailOpen(true);
            }}
          />
        )}
        {showDetail && (
          <div className="home-inflow-review__selection">
            <aside
              className="home-inflow-review__queue"
              aria-labelledby={queueTitleId}
            >
              <div className="home-inflow-review__queue-heading">
                <strong id={queueTitleId}>
                  {view === "calendar"
                    ? copy.projects.inflowHomeReceivedOn(dateLabel)
                    : copy.projects.inflowHomeQueueTitle}
                </strong>
                <span>{visibleItems.length}</span>
                {view === "calendar" && (
                  <button
                    type="button"
                    className="home-inflow-review__close focus-visible-control"
                    aria-label={copy.projects.inflowHomeCloseDetail}
                    onClick={() => {
                      setCalendarDetailOpen(false);
                      reviewRef.current
                        ?.querySelector<HTMLButtonElement>(
                          '.planning-calendar__day[aria-pressed="true"], .planning-week__date[aria-pressed="true"]',
                        )
                        ?.focus({ preventScroll: true });
                    }}
                  >
                    <X aria-hidden="true" />
                  </button>
                )}
              </div>
              <ol ref={queueRef}>
                {visibleItems.map((item) => {
                  const conversationId = inflowConversationKey(item);
                  const active =
                    conversationId ===
                    (selectedItem
                      ? inflowConversationKey(selectedItem)
                      : undefined);
                  return (
                    <li key={conversationId}>
                      <button
                        className="home-inflow-review__queue-item focus-visible-control"
                        type="button"
                        aria-pressed={active}
                        data-active={active}
                        onClick={() =>
                          setSelectedConversationId(conversationId)
                        }
                      >
                        <span className="home-inflow-review__queue-meta">
                          <strong>
                            {item.senderName ??
                              copy.projects.inflowSenderPending}
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
              {queueOverflows && (
                <p className="home-inflow-review__scroll-hint">
                  <ChevronDown aria-hidden="true" />
                  {copy.projects.inflowHomeScrollHint}
                </p>
              )}
            </aside>

            {selectedItem ? (
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
            ) : (
              <div className="home-inflow-review__empty" role="status">
                <strong>{copy.projects.inflowHomeDateEmpty}</strong>
                <p>{copy.projects.inflowHomeDateEmptyHelp}</p>
              </div>
            )}
          </div>
        )}
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
  if (Number.isNaN(date.getTime())) return copy.projects.inflowHomeDatePending;
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
