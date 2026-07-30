import { ChevronDown, MessageCircleMore } from "lucide-react";
import { useMemo, useState } from "react";

import { type ProjectInflowItem } from "../api/googleChat";
import { copy } from "../copy";
import {
  InflowItemRow,
  inflowConversationKey,
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
  const visibleItems = useMemo(() => items.slice(0, 5), [items]);
  const [selectedConversationId, setSelectedConversationId] = useState(
    visibleItems[0] ? inflowConversationKey(visibleItems[0]) : undefined,
  );
  const [mobileExpanded, setMobileExpanded] = useState(false);
  const selectedItem =
    visibleItems.find(
      (item) => inflowConversationKey(item) === selectedConversationId,
    ) ?? visibleItems[0];

  if (!selectedItem) return <></>;

  return (
    <section
      className="home-inflow"
      aria-labelledby="home-inflow-title"
      data-mobile-expanded={mobileExpanded}
    >
      <header className="home-inflow__heading">
        <div className="home-inflow__heading-copy">
          <span>{copy.projects.inflowHomeEyebrow}</span>
          <h2 id="home-inflow-title">{copy.projects.inflowHomeTitle}</h2>
          <p>{copy.projects.inflowHomeDescription}</p>
        </div>
        <strong aria-label={`${items.length}개의 업무 요청`}>
          {items.length}
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
            ? copy.projects.inflowHomeCollapse
            : copy.projects.inflowHomeOpen(items.length)}
        </span>
        <ChevronDown aria-hidden="true" />
      </button>

      <div className="home-inflow-review">
        <aside
          className="home-inflow-review__queue"
          aria-labelledby="home-inflow-queue-title"
        >
          <div className="home-inflow-review__queue-heading">
            <MessageCircleMore aria-hidden="true" />
            <strong id="home-inflow-queue-title">
              {copy.projects.inflowHomeQueueTitle}
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
        </aside>

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
      </div>
    </section>
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
