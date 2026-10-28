import { useCallback, useEffect, useRef, useState } from "react";
import { CheckCheck, ChevronDown, RefreshCw } from "lucide-react";
import {
  completionReviewRequest,
  type CompletionReview,
  type CompletionReviewPage,
} from "../api/completionReviews";
import { PlanningRequestError } from "../api/planning";
import { completionReviewCopy as c } from "../copy/completionReviews";
import "./completionReviews.css";

interface Props {
  baseUrl: string;
  authenticate<T>(request: (token: string) => Promise<T>): Promise<T>;
  onChanged(): void | Promise<void>;
  highlightedId?: string;
}
export function CompletionReviewPanel({
  baseUrl,
  authenticate,
  onChanged,
  highlightedId,
}: Props) {
  const [items, setItems] = useState<CompletionReview[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [busy, setBusy] = useState<string>();
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState("");
  const generation = useRef(0);
  const pending = useRef(false);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(""), 6000);
    return () => window.clearTimeout(timer);
  }, [notice]);
  const load = useCallback(
    async (before?: string) => {
      const epoch = ++generation.current;
      try {
        const page = await authenticate((token) =>
          completionReviewRequest<CompletionReviewPage>(
            baseUrl,
            token,
            before ? `?before=${encodeURIComponent(before)}` : "",
          ),
        );
        if (epoch !== generation.current) return;
        setItems((old) =>
          before
            ? [
                ...old.filter((r) => !page.items.some((n) => n.id === r.id)),
                ...page.items,
              ]
            : page.items,
        );
        setCursor(page.nextCursor);
        setError(undefined);
      } catch {
        if (epoch === generation.current) setError(c.failed);
      }
    },
    [authenticate, baseUrl],
  );
  useEffect(() => {
    void load();
    const refresh = () => {
      if (!pending.current && document.visibilityState !== "hidden")
        void load();
    };
    const timer = window.setInterval(refresh, 30_000);
    document.addEventListener("visibilitychange", refresh);
    window.addEventListener("focus", refresh);
    window.addEventListener("completion-reviews-changed", refresh);
    return () => {
      generation.current++;
      clearInterval(timer);
      document.removeEventListener("visibilitychange", refresh);
      window.removeEventListener("focus", refresh);
      window.removeEventListener("completion-reviews-changed", refresh);
    };
  }, [load]);
  useEffect(() => {
    if (!highlightedId) return;
    setExpanded(true);
    void load();
  }, [highlightedId, load]);
  useEffect(() => {
    if (highlightedId && items.some((r) => r.id === highlightedId))
      document
        .getElementById(`completion-review-${highlightedId}`)
        ?.scrollIntoView({ block: "nearest" });
  }, [highlightedId, items]);
  async function decide(
    review: CompletionReview,
    approve: boolean,
    note: string,
  ) {
    if (pending.current) return;
    pending.current = true;
    generation.current++;
    setBusy(review.id);
    setError(undefined);
    setNotice("");
    try {
      await authenticate((token) =>
        completionReviewRequest(baseUrl, token, `/${review.id}/decision`, {
          kind: approve ? "approve" : "reject",
          expectedVersion: review.version,
          expectedTaskVersion: review.taskVersion,
          note: note.trim() || null,
        }),
      );
      setItems((old) => old.filter((r) => r.id !== review.id));
      setNotice(approve ? c.approved : c.rejected);
      await onChanged();
    } catch (e) {
      if (e instanceof PlanningRequestError && e.code === "conflict") {
        await load();
        setError(c.conflict);
      } else
        setError(
          e instanceof PlanningRequestError && e.code === "invalid"
            ? c.blocked
            : c.saveNotice,
        );
    } finally {
      pending.current = false;
      setBusy(undefined);
    }
  }
  if (!items.length && !error && !notice) return null;
  return (
    <section className="completion-reviews" aria-label={c.title}>
      <header>
        <div>
          <h2>
            <CheckCheck size={20} aria-hidden="true" />
            {c.title}{" "}
            {items.length > 0 && (
              <span>
                {items.length}
                {cursor ? "+" : ""}개
              </span>
            )}
          </h2>
          <p>{c.help}</p>
        </div>
        <button
          type="button"
          className="button button--ghost"
          disabled={Boolean(busy)}
          onClick={() => void load()}
          aria-label={c.retry}
        >
          <RefreshCw size={18} />
        </button>
      </header>
      {error && (
        <p role="alert" className="inline-alert">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
      {(expanded ? items : items.slice(0, 3)).map((review) => (
        <CompletionReviewCard
          key={review.id}
          review={review}
          busy={Boolean(busy)}
          saving={busy === review.id}
          initiallyOpen={highlightedId === review.id}
          onDecide={decide}
        />
      ))}
      {items.length > 3 && (
        <button
          className="button button--ghost"
          type="button"
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? c.collapse : c.expand}
        </button>
      )}
      {expanded && cursor && (
        <button
          className="button button--ghost"
          type="button"
          disabled={loadingMore || Boolean(busy)}
          onClick={async () => {
            setLoadingMore(true);
            await load(cursor);
            setLoadingMore(false);
          }}
        >
          {c.more}
        </button>
      )}
    </section>
  );
}

export function CompletionReviewCard({
  review: currentReview,
  busy,
  saving,
  initiallyOpen,
  onDecide,
}: {
  review: CompletionReview;
  busy: boolean;
  saving: boolean;
  initiallyOpen: boolean;
  onDecide(
    review: CompletionReview,
    approve: boolean,
    note: string,
  ): Promise<void>;
}) {
  const [open, setOpen] = useState(initiallyOpen);
  const [review, setReview] = useState(currentReview);
  const [note, setNote] = useState("");
  const outdated =
    open &&
    (review.taskVersion !== currentReview.taskVersion ||
      review.version !== currentReview.version);
  useEffect(() => {
    if (!open) setReview(currentReview);
  }, [currentReview, open]);
  useEffect(() => {
    if (initiallyOpen) setOpen(true);
  }, [initiallyOpen]);
  return (
    <article
      id={`completion-review-${review.id}`}
      className="completion-review"
      aria-busy={saving}
    >
      <button
        className="completion-review__toggle"
        type="button"
        aria-expanded={open}
        aria-controls={`review-body-${review.id}`}
        onClick={() => {
          if (!open) setReview(currentReview);
          setOpen(!open);
        }}
      >
        <span>
          <strong>{review.title}</strong>
          <small>
            {review.projectTitle} · {review.reporterName}님의 완료 보고
          </small>
        </span>
        <ChevronDown size={18} aria-hidden="true" />
      </button>
      {open && (
        <div
          className="completion-review__body"
          id={`review-body-${review.id}`}
        >
          {outdated && (
            <p className="inline-alert" role="status">
              {c.newer}
              <button
                type="button"
                className="button button--ghost"
                onClick={() => setReview(currentReview)}
              >
                {c.readLatest}
              </button>
            </p>
          )}
          {review.reportedTaskVersion !== review.taskVersion && (
            <p role="status" className="inline-alert">
              {c.changed}
            </p>
          )}
          <strong>{c.note}</strong>
          <p className="completion-review__text">
            {review.completionNote || c.noNote}
          </p>
          <details>
            <summary>{c.task}</summary>
            <p className="completion-review__text">
              {review.taskNotes || review.title}
            </p>
            <p>담당자: {review.assigneeName || "미정"}</p>
            <p>
              마감:{" "}
              {review.dueAt
                ? new Date(review.dueAt).toLocaleString("ko-KR")
                : "정하지 않음"}
            </p>
          </details>
          <label htmlFor={`review-note-${review.id}`}>{c.reason}</label>
          <textarea
            id={`review-note-${review.id}`}
            value={note}
            onChange={(e) => setNote(e.target.value)}
            maxLength={2000}
            placeholder={c.reasonHelp}
            disabled={busy}
            rows={2}
          />
          <div className="completion-review__actions">
            <button
              type="button"
              className="button button--secondary"
              disabled={busy || outdated || !note.trim()}
              onClick={() => void onDecide(review, false, note)}
            >
              {c.reject}
            </button>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || outdated}
              onClick={() => void onDecide(review, true, "")}
            >
              {saving ? c.processing : c.approve}
            </button>
          </div>
        </div>
      )}
    </article>
  );
}
