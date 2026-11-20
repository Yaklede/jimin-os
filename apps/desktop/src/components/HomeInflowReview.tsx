import { ArrowLeft, ChevronDown, X } from "lucide-react";
import { type ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { type ProjectInflowItem } from "../api/googleChat";
import { registerMobileBackHandler } from "../mobileBack";
import { copy } from "../copy";
import { completionReviewCopy } from "../copy/completionReviews";
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
  completionReviews?: ReactNode;
  highlightedCompletionReviewId?: string;
  onFocusHandled?(id: number): void;
  focusRequest?: number;
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
  onReviewChange?(item: ProjectInflowItem): void;
  onOpenTask(taskId: string): Promise<void>;
};

const HOME_INFLOW_PREVIEW_SIZE = 4;

function useSinglePaneInflow() {
  const query = "(max-width: 1000px)";
  const [singlePane, setSinglePane] = useState(
    () => typeof window !== "undefined" && window.matchMedia(query).matches,
  );
  useEffect(() => {
    const media = window.matchMedia(query);
    const update = () => setSinglePane(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  return singlePane;
}

export function HomeInflowReview({ ...props }: HomeInflowReviewProps) {
  const pending = homeInflowPendingItems(props.items);
  const unread = pending.filter((item) => !item.reviewed);
  const groups = {
    new: unread.filter(
      (item) => !item.promotedTaskId && !isUncertainInflow(item),
    ),
    existing: unread.filter((item) => item.promotedTaskId),
    uncertain: unread.filter(isUncertainInflow),
  };
  type InflowTab = keyof typeof groups | "completion";
  const tabs: InflowTab[] = ["new", "existing", "uncertain"];
  const hasCompletion = Boolean(props.completionReviews);
  if (hasCompletion) tabs.push("completion");
  const [activeTab, setActiveTab] = useState<InflowTab>(() =>
    props.highlightedCompletionReviewId && hasCompletion
      ? "completion"
      : groups.new.length > 0 || (hasCompletion && unread.length === 0)
      ? "new"
      : groups.existing.length > 0
        ? "existing"
        : "uncertain",
  );
  const focusTarget = useRef<HTMLDivElement>(null);
  const emptyRequestRef = useRef<HTMLParagraphElement>(null);
  const handledFocus = useRef<number | undefined>(undefined);
  const highlightedCompletionId = props.highlightedCompletionReviewId;
  useEffect(() => {
    if (!highlightedCompletionId || !hasCompletion) return;
    setActiveTab("completion");
  }, [highlightedCompletionId, hasCompletion]);
  const receiptCount = useRef(0);
  const [reviewReceipt, setReviewReceipt] = useState<{
    id: number;
    message: string;
  }>();
  const showReceipt = (message: string) =>
    setReviewReceipt({ id: ++receiptCount.current, message });
  const handleDismiss: HomeInflowReviewProps["onDismiss"] = async (
    item,
    input,
  ) => {
    await props.onDismiss(item, input);
    if (input?.markSeen) showReceipt(copy.projects.inflowReviewMoved);
  };

  const handlePromote: HomeInflowReviewProps["onPromote"] = async (
    item,
    input,
  ) => {
    await props.onPromote(item, input);
    showReceipt(copy.projects.inflowRegistrationSaved);
  };

  useEffect(() => {
    if (!reviewReceipt) return;
    if (document.activeElement === document.body) {
      const fallback =
        focusTarget.current?.querySelector<HTMLButtonElement>(
          'button[role="tab"][aria-selected="true"]',
        ) ?? emptyRequestRef.current;
      fallback?.focus({ preventScroll: true });
    }
    const timer = window.setTimeout(() => setReviewReceipt(undefined), 6_000);
    return () => window.clearTimeout(timer);
  }, [reviewReceipt]);
  const receipt =
    typeof document === "undefined"
      ? null
      : createPortal(
          <div
            className="home-inflow-review__receipt"
            role="status"
            aria-atomic="true"
          >
            {reviewReceipt && (
              <p key={reviewReceipt.id}>{reviewReceipt.message}</p>
            )}
          </div>,
          document.body,
        );
  useEffect(() => {
    if (props.focusRequest && handledFocus.current !== props.focusRequest) {
      handledFocus.current = props.focusRequest;
      props.onFocusHandled?.(props.focusRequest);
      setActiveTab(
        groups.new.length
          ? "new"
          : groups.existing.length
            ? "existing"
            : "uncertain",
      );
      focusTarget.current?.scrollIntoView({ block: "start" });
    }
  }, [
    props.focusRequest,
    groups.new.length,
    groups.existing.length,
    props.onFocusHandled,
  ]);
  if (unread.length === 0 && !hasCompletion) {
    return (
      <>
        {receipt}
        <p
          ref={emptyRequestRef}
          tabIndex={-1}
          className="home-inflow-tabs__empty"
          role="status"
        >
          {copy.home.inboxClear}
        </p>
      </>
    );
  }
  return (
    <>
      {receipt}
      <div className="inflow-review-queues" ref={focusTarget}>
        <div
          className="home-inflow-tabs"
          role="tablist"
          aria-label={copy.projects.inflowTabsLabel}
          data-completion={hasCompletion || undefined}
        >
          {tabs.map((kind, index) => (
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
                      ? tabs.length - 1
                      : event.key === "ArrowRight"
                        ? (index + 1) % tabs.length
                        : event.key === "ArrowLeft"
                          ? (index + tabs.length - 1) % tabs.length
                          : null;
                if (next === null) return;
                event.preventDefault();
                setActiveTab(tabs[next]);
                event.currentTarget.parentElement
                  ?.querySelectorAll<HTMLButtonElement>('[role="tab"]')
                  [next]?.focus();
              }}
            >
              <span className="home-inflow-tabs__full-label">
                {kind === "new"
                  ? copy.projects.inflowNewTitle
                  : kind === "completion"
                    ? completionReviewCopy.tab
                    : kind === "existing"
                    ? copy.projects.inflowExistingTitle
                    : copy.projects.inflowUncertainTitle}
              </span>
              <span className="home-inflow-tabs__short-label">
                {kind === "new"
                  ? copy.projects.inflowNewTabShort
                  : kind === "completion"
                    ? completionReviewCopy.tab
                    : kind === "existing"
                    ? copy.projects.inflowExistingTabShort
                    : copy.projects.inflowUncertainTitle}
              </span>
              {kind !== "completion" && (
                <span className="home-inflow-tabs__count">{groups[kind].length}</span>
              )}
            </button>
          ))}
        </div>
        {tabs.map((kind) => (
          <div
            key={kind}
            className="home-inflow-tab-panel"
            id={`home-inflow-panel-${kind}`}
            role="tabpanel"
            aria-labelledby={`home-inflow-tab-${kind}`}
            hidden={activeTab !== kind}
            tabIndex={0}
          >
            {kind === "completion" ? props.completionReviews : groups[kind].length > 0 ? (
              <HomeInflowGroup
                {...props}
                items={groups[kind]}
                kind={kind}
                onPromote={handlePromote}
                onDismiss={handleDismiss}
              />
            ) : (
              <p className="home-inflow-tabs__empty">
                {kind === "new"
                  ? copy.projects.inflowNewEmpty
                  : kind === "existing"
                    ? copy.projects.inflowExistingEmpty
                    : copy.projects.inflowUncertainEmpty}
              </p>
            )}
          </div>
        ))}
      </div>
    </>
  );
}

function HomeInflowGroup({
  items,
  saving,
  onPromote,
  onDismiss,
  onRetryAnalysis,
  onRetryCompletion,
  onReviewChange,
  onOpenTask,
  kind,
  initialView = "list",
}: HomeInflowReviewProps & { kind: "new" | "existing" | "uncertain" }) {
  const mobile = useSinglePaneInflow();
  const [mobileDetailOpen, setMobileDetailOpen] = useState(false);
  const titleId = `home-inflow-title-${kind}`;
  const detailId = `home-inflow-detail-title-${kind}`;
  const queueTitleId = `home-inflow-queue-title-${kind}`;
  const queueCountId = `home-inflow-queue-count-${kind}`;
  const reviewRef = useRef<HTMLDivElement>(null);
  const queueRef = useRef<HTMLOListElement>(null);
  const calendarResultRef = useRef<HTMLElement>(null);
  const detailRef = useRef<HTMLElement>(null);
  const mobileBackRef = useRef<HTMLButtonElement>(null);
  const singleDetailWasOpen = useRef(false);
  const returnQueue = useRef({ scrollTop: 0, rowIndex: 0 });
  const [queueOverflows, setQueueOverflows] = useState(false);
  const allItems = useMemo(
    () => homeInflowByReceivedDate(items.filter(isProjectInflowAttentionItem)),
    [items],
  );
  const preferenceKey = `home-inflow-view-${kind}`;
  const [view, setView] = useState<"list" | "calendar">(() => {
    try {
      const stored = window.sessionStorage.getItem(preferenceKey);
      return stored === "calendar" || stored === "list" ? stored : initialView;
    } catch {
      return initialView;
    }
  });
  const previousDetailView = useRef(view);
  const [showAll, setShowAll] = useState(false);
  const [range, setRange] = useState(() => planningViewRange("month"));
  const [calendarDetailOpen, setCalendarDetailOpen] = useState(false);
  const [calendarReveal, setCalendarReveal] = useState(0);
  const handledCalendarReveal = useRef(0);
  const visibleItems = useMemo(
    () =>
      view === "calendar" ? homeInflowOnDate(allItems, range.anchor) : allItems,
    [allItems, view, range.anchor],
  );
  const showDetail = view === "list" || calendarDetailOpen;
  const calendarDay = view === "calendar" ? range.anchor.getTime() : undefined;

  useLayoutEffect(() => {
    returnQueue.current = { scrollTop: 0, rowIndex: 0 };
    if (queueRef.current) queueRef.current.scrollTop = 0;
  }, [view, calendarDay, showDetail]);

  useEffect(() => {
    const queue = queueRef.current;
    if (!queue || !showDetail) return;
    const measure = () => {
      setQueueOverflows(
        queue.clientHeight > 0 && queue.scrollHeight > queue.clientHeight + 1,
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(queue);
    for (const row of queue.children) observer.observe(row);
    return () => observer.disconnect();
  }, [visibleItems, showDetail, showAll]);
  const [selectedConversationId, setSelectedConversationId] =
    useState<string>();

  const selectedItem =
    visibleItems.find(
      (item) => inflowConversationKey(item) === selectedConversationId,
    ) ?? (view === "calendar" ? visibleItems[0] : undefined);
  const queueItems = visibleHomeInflowItems(
    visibleItems,
    view === "calendar" || showAll,
  );
  const displayedItem = !mobile || mobileDetailOpen ? selectedItem : undefined;
  const activeConversation = displayedItem
    ? inflowConversationKey(displayedItem)
    : undefined;
  const isSingleDetail = mobile && Boolean(showDetail && displayedItem);

  // Run after the calendar's own focus restoration so the narrow result gets focus.
  useEffect(() => {
    if (handledCalendarReveal.current === calendarReveal) return;
    handledCalendarReveal.current = calendarReveal;
    if (!mobile || view !== "calendar") return;
    const result = calendarResultRef.current;
    if (!result?.getClientRects().length) return;
    result.scrollIntoView({
      block: "start",
      inline: "nearest",
      behavior: "instant",
    });
    result.focus({ preventScroll: true });
  }, [calendarReveal, mobile, view]);

  useEffect(() => {
    if (!isSingleDetail) return;
    return registerMobileBackHandler(() => {
      if (!reviewRef.current?.getClientRects().length) return false;
      setMobileDetailOpen(false);
      return true;
    }, 60);
  }, [isSingleDetail]);

  useLayoutEffect(() => {
    const changedView = previousDetailView.current !== view;
    previousDetailView.current = view;
    const detail = detailRef.current;
    if (!detail?.getClientRects().length) return;
    detail.scrollTop = 0;
    if (!mobile && !changedView) {
      detail.firstElementChild?.scrollIntoView({
        block: "nearest",
        inline: "nearest",
        behavior: "instant",
      });
    }
  }, [activeConversation, mobile, view]);

  useLayoutEffect(() => {
    const wasOpen = singleDetailWasOpen.current;
    singleDetailWasOpen.current = isSingleDetail;
    if (isSingleDetail && !wasOpen) {
      const detail = detailRef.current;
      if (!detail?.getClientRects().length) return;
      detail.scrollIntoView({
        block: "start",
        inline: "nearest",
        behavior: "instant",
      });
      mobileBackRef.current?.focus({ preventScroll: true });
    } else if (!isSingleDetail && wasOpen) {
      const queue = queueRef.current;
      if (!queue?.getClientRects().length) return;
      const { scrollTop, rowIndex } = returnQueue.current;
      queue.scrollTop = scrollTop;
      const rows = queue.querySelectorAll<HTMLButtonElement>("button");
      const row =
        queue.querySelector<HTMLButtonElement>('[aria-pressed="true"]') ??
        rows[Math.min(rowIndex, rows.length - 1)];
      row?.scrollIntoView({
        block: "nearest",
        inline: "nearest",
        behavior: "instant",
      });
      row?.focus({ preventScroll: true });
    }
  }, [isSingleDetail]);

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
              : kind === "existing"
                ? copy.projects.inflowExistingEyebrow
                : "AI 분류 확인"}
          </span>
          <h2 id={titleId}>
            {kind === "new"
              ? copy.projects.inflowNewTitle
              : kind === "existing"
                ? copy.projects.inflowExistingTitle
                : copy.projects.inflowUncertainTitle}
          </h2>
          <p>
            {kind === "new"
              ? copy.projects.inflowHomeDescription
              : kind === "existing"
                ? copy.projects.inflowExistingDescription
                : copy.projects.inflowUncertainDescription}
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
            onClick={() => {
              if (view === mode) return;
              setView(mode);
              setSelectedConversationId(undefined);
              setMobileDetailOpen(false);
              try {
                window.sessionStorage.setItem(preferenceKey, mode);
              } catch {
                // The current view remains usable when storage is unavailable.
              }
            }}
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
              setMobileDetailOpen(false);
              setCalendarDetailOpen(true);
              setCalendarReveal((reveal) => reveal + 1);
            }}
            onSelectDate={(date) => {
              setRange(planningViewRange(range.mode, date));
              setSelectedConversationId(undefined);
              setMobileDetailOpen(false);
              setCalendarDetailOpen(true);
              setCalendarReveal((reveal) => reveal + 1);
            }}
          />
        )}
        {showDetail && (
          <div
            className="home-inflow-review__selection"
            data-mobile-detail={isSingleDetail}
            data-has-selection={Boolean(displayedItem)}
          >
            <aside
              ref={view === "calendar" ? calendarResultRef : undefined}
              className="home-inflow-review__queue"
              tabIndex={view === "calendar" ? -1 : undefined}
              aria-labelledby={
                view === "calendar"
                  ? `${queueTitleId} ${queueCountId}`
                  : queueTitleId
              }
            >
              <div className="home-inflow-review__queue-heading">
                <strong id={queueTitleId}>
                  {view === "calendar"
                    ? copy.projects.inflowHomeReceivedOn(dateLabel)
                    : copy.projects.inflowHomeQueueTitle}
                </strong>
                <span id={queueCountId}>
                  {view === "calendar"
                    ? `${visibleItems.length}건`
                    : visibleItems.length}
                </span>
                {view === "calendar" && (
                  <button
                    type="button"
                    className="home-inflow-review__close focus-visible-control"
                    aria-label={copy.projects.inflowHomeCloseDetail}
                    onClick={() => {
                      setCalendarDetailOpen(false);
                      const selectedDate =
                        reviewRef.current?.querySelector<HTMLButtonElement>(
                          '.planning-calendar__day[aria-pressed="true"], .planning-week__date[aria-pressed="true"]',
                        );
                      if (mobile) {
                        selectedDate?.scrollIntoView({
                          block: "nearest",
                          inline: "nearest",
                          behavior: "instant",
                        });
                      }
                      selectedDate?.focus({ preventScroll: true });
                    }}
                  >
                    <X aria-hidden="true" />
                  </button>
                )}
              </div>
              <ol ref={queueRef}>
                {queueItems.map((item, rowIndex) => {
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
                        onClick={() => {
                          returnQueue.current = {
                            scrollTop: queueRef.current?.scrollTop ?? 0,
                            rowIndex,
                          };
                          setSelectedConversationId(conversationId);
                          setMobileDetailOpen(true);
                        }}
                      >
                        <span className="home-inflow-review__queue-summary">
                          <span className="home-inflow-review__queue-title">
                            {item.suggestedTaskTitle}
                          </span>
                          <span className="home-inflow-review__queue-origin">
                            {kind === "existing" && (
                              <span>
                                {copy.projects.inflowExistingRowBadge}
                              </span>
                            )}
                            <small>{item.sourceName}</small>
                          </span>
                        </span>
                        <span className="home-inflow-review__queue-meta">
                          <strong>
                            {item.senderName ??
                              copy.projects.inflowSenderPending}
                          </strong>
                          <time dateTime={item.receivedAt}>
                            {formatHomeInflowTime(item.receivedAt)}
                          </time>
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ol>
              {view === "list" &&
                visibleItems.length > HOME_INFLOW_PREVIEW_SIZE && (
                  <button
                    type="button"
                    className="home-inflow-review__expand text-button focus-visible-control"
                    aria-expanded={showAll}
                    onClick={() => {
                      if (
                        showAll &&
                        !visibleHomeInflowItems(visibleItems, false).some(
                          (item) =>
                            inflowConversationKey(item) ===
                            selectedConversationId,
                        )
                      ) {
                        setSelectedConversationId(undefined);
                        setMobileDetailOpen(false);
                      }
                      setShowAll(!showAll);
                    }}
                  >
                    {showAll
                      ? copy.home.inboxLess
                      : copy.home.inboxMore(
                          visibleItems.length - HOME_INFLOW_PREVIEW_SIZE,
                        )}
                  </button>
                )}
              {queueOverflows && (
                <p className="home-inflow-review__scroll-hint">
                  <ChevronDown aria-hidden="true" />
                  {copy.projects.inflowHomeScrollHint}
                </p>
              )}
            </aside>

            {displayedItem ? (
              <section
                ref={detailRef}
                className="home-inflow-review__detail"
                aria-labelledby={detailId}
              >
                <header>
                  {mobile && (
                    <button
                      ref={mobileBackRef}
                      type="button"
                      className="secondary-button focus-visible-control"
                      onClick={() => {
                        setMobileDetailOpen(false);
                      }}
                    >
                      <ArrowLeft aria-hidden="true" />
                      요청 목록으로
                    </button>
                  )}
                  <span>{copy.projects.inflowHomeSelectedLabel}</span>
                  <strong id={detailId}>
                    {copy.projects.inflowHomeSelectedRequest(
                      displayedItem.senderName || "",
                    )}
                  </strong>
                </header>
                <ul>
                  <InflowItemRow
                    key={inflowConversationKey(displayedItem)}
                    inlinePromotion
                    item={displayedItem}
                    saving={saving}
                    onPromote={onPromote}
                    onDismiss={onDismiss}
                    onRetryAnalysis={onRetryAnalysis}
                    onRetryCompletion={onRetryCompletion}
                    onReviewChange={onReviewChange}
                    onOpenTask={(taskId) => void onOpenTask(taskId)}
                  />
                </ul>
              </section>
            ) : (
              view === "calendar" &&
              !selectedItem && (
                <div className="home-inflow-review__empty" role="status">
                  <strong>{copy.projects.inflowHomeDateEmpty}</strong>
                  <p>{copy.projects.inflowHomeDateEmptyHelp}</p>
                </div>
              )
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

export function isUncertainInflow(item: ProjectInflowItem): boolean {
  return (
    !item.promotedTaskId &&
    (item.analysisStatus === "failed" ||
      (item.analysisStatus === "ready" &&
        item.analysisClassification !== "new_task"))
  );
}

export function visibleHomeInflowItems(
  items: ProjectInflowItem[],
  showAll: boolean,
): ProjectInflowItem[] {
  return showAll ? items : items.slice(0, HOME_INFLOW_PREVIEW_SIZE);
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
