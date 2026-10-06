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
  inflowConversationKey,
  isProjectInflowAttentionItem,
  type PromoteInflowInput,
} from "./ProjectInflowPanel";

type HomeInflowReviewProps = {
  items: ProjectInflowItem[];
  saving: boolean;
  onPromote(item: ProjectInflowItem, input: PromoteInflowInput): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
  onRetryAnalysis(item: ProjectInflowItem): Promise<void>;
  onRetryCompletion(item: ProjectInflowItem): Promise<void>;
};

export function HomeInflowReview({
  items,
  saving,
  onPromote,
  onDismiss,
  onRetryAnalysis,
  onRetryCompletion,
}: HomeInflowReviewProps) {
  const reviewRef = useRef<HTMLDivElement>(null);
  const queueRef = useRef<HTMLOListElement>(null);
  const [queueOverflows, setQueueOverflows] = useState(false);
  const allItems = useMemo(
    () => homeInflowByReceivedDate(items.filter(isProjectInflowAttentionItem)),
    [items],
  );
  const [view, setView] = useState<"list" | "calendar">("calendar");
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
      aria-labelledby="home-inflow-title"
      data-view={view}
      data-detail-open={showDetail}
    >
      <header className="home-inflow__heading">
        <div className="home-inflow__heading-copy">
          <span>{copy.projects.inflowHomeEyebrow}</span>
          <h2 id="home-inflow-title">{copy.projects.inflowHomeTitle}</h2>
          <p>{copy.projects.inflowHomeDescription}</p>
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
              aria-labelledby="home-inflow-queue-title"
            >
              <div className="home-inflow-review__queue-heading">
                <strong id="home-inflow-queue-title">
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
                aria-labelledby="home-inflow-detail-title"
              >
                <header>
                  <span>{copy.projects.inflowHomeSelectedLabel}</span>
                  <strong id="home-inflow-detail-title">
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

function formatHomeInflowTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return copy.projects.inflowHomeDatePending;
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
