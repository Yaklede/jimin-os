import { useCallback, useEffect, useRef, useState } from "react";
import {
  CheckCheck,
  CheckCircle2,
  ChevronDown,
  Circle,
  CircleMinus,
  RefreshCw,
} from "lucide-react";
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

function ReviewCheckbox({
  label,
  descriptionId,
  checked,
  mixed = false,
  disabled,
  onChange,
}: {
  label: string;
  descriptionId?: string;
  checked: boolean;
  mixed?: boolean;
  disabled: boolean;
  onChange(checked: boolean): void;
}) {
  const Mark = mixed ? CircleMinus : checked ? CheckCircle2 : Circle;
  return (
    <span className="completion-reviews__pick-control">
      <input
        type="checkbox"
        aria-label={label}
        aria-describedby={descriptionId}
        checked={checked}
        ref={(input) => {
          if (input) input.indeterminate = mixed;
        }}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <Mark aria-hidden="true" />
    </span>
  );
}

export function canConfirmTogether(review: CompletionReview) {
  return review.reportedTaskVersion === review.taskVersion;
}

export function sameReviewVersion(a: CompletionReview, b: CompletionReview) {
  return (
    a.id === b.id &&
    a.version === b.version &&
    a.taskVersion === b.taskVersion &&
    a.reportedTaskVersion === b.reportedTaskVersion
  );
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
  const [loaded, setLoaded] = useState(false);
  const [selected, setSelected] = useState<CompletionReview[]>([]);
  const [batch, setBatch] = useState<{ done: number; total: number }>();
  const heading = useRef<HTMLHeadingElement>(null);
  const confirmButton = useRef<HTMLButtonElement>(null);
  const generation = useRef(0);
  const pending = useRef(false);
  const working = Boolean(busy || batch);
  const visible = expanded ? items : items.slice(0, 3);
  const eligibleVisible = visible.filter(canConfirmTogether);
  const currentSelection = selected.filter((snapshot) =>
    items.some(
      (item) => canConfirmTogether(item) && sameReviewVersion(item, snapshot),
    ),
  );
  const selectedIds = new Set(currentSelection.map((item) => item.id));
  const selectedVisible = eligibleVisible.filter((item) =>
    selectedIds.has(item.id),
  );
  const allVisibleSelected =
    eligibleVisible.length > 0 &&
    selectedVisible.length === eligibleVisible.length;
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
      } finally {
        if (epoch === generation.current) setLoaded(true);
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
  const submitDecision = (
    review: CompletionReview,
    approve: boolean,
    note: string,
  ) =>
    authenticate((token) =>
      completionReviewRequest(baseUrl, token, `/${review.id}/decision`, {
        kind: approve ? "approve" : "reject",
        expectedVersion: review.version,
        expectedTaskVersion: review.taskVersion,
        note: note.trim() || null,
      }),
    );

  function forgetDecisions(ids: ReadonlySet<string>) {
    setItems((current) => current.filter((item) => !ids.has(item.id)));
    setSelected((current) => current.filter((item) => !ids.has(item.id)));
  }

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
      await submitDecision(review, approve, note);
      forgetDecisions(new Set([review.id]));
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

  async function approveSelected() {
    if (pending.current || !currentSelection.length) return;
    const targets = currentSelection;
    const returnFocus = document.activeElement === confirmButton.current;
    const approved = new Set<string>();
    let conflict = false;
    pending.current = true;
    generation.current++;
    setError(undefined);
    setNotice("");
    setSelected(targets);
    setBatch({ done: 0, total: targets.length });
    try {
      // Each report retains its own task/review versions and normal decision side effects.
      for (const [index, review] of targets.entries()) {
        try {
          await submitDecision(review, true, "");
          approved.add(review.id);
        } catch (failure) {
          if (
            failure instanceof PlanningRequestError &&
            failure.code === "conflict"
          ) {
            conflict = true;
          }
        }
        setBatch({ done: index + 1, total: targets.length });
      }
      forgetDecisions(approved);
      if (conflict) await load();
      setNotice(c.batchResult(approved.size, targets.length - approved.size));
      if (approved.size) {
        try {
          await onChanged();
        } catch {
          // Confirmed reports are already saved; a later refresh can reconcile the home list.
        }
      }
    } finally {
      pending.current = false;
      setBatch(undefined);
      if (
        returnFocus &&
        (document.activeElement === document.body ||
          document.activeElement === confirmButton.current) &&
        heading.current?.getClientRects().length
      ) {
        heading.current.focus({ preventScroll: true });
      }
    }
  }

  return (
    <section className="completion-reviews" aria-label={c.title}>
      <header>
        <div>
          <h2 ref={heading} tabIndex={-1}>
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
          className="button button--ghost focus-visible-control"
          disabled={working}
          onClick={() => void load()}
          aria-label={c.retry}
        >
          <RefreshCw size={18} aria-hidden="true" />
        </button>
      </header>
      {error && (
        <p role="alert" className="inline-alert">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
      {!loaded && !error && <p role="status">{c.loading}</p>}
      {loaded && !items.length && !error && <p>{c.empty}</p>}
      {items.length > 0 && (
        <div className="completion-reviews__selection">
          <label className="completion-reviews__pick">
            <ReviewCheckbox
              label={c.selectVisible}
              checked={allVisibleSelected}
              mixed={selectedVisible.length > 0 && !allVisibleSelected}
              disabled={working || !eligibleVisible.length}
              onChange={(checked) => {
                const visibleIds = new Set(
                  eligibleVisible.map((item) => item.id),
                );
                const otherSelections = currentSelection.filter(
                  (item) => !visibleIds.has(item.id),
                );
                setSelected(
                  checked
                    ? [...otherSelections, ...eligibleVisible]
                    : otherSelections,
                );
              }}
            />
            {c.selectVisible}
          </label>
          <span className="completion-reviews__count" role="status">
            {batch
              ? c.confirming(batch.done, batch.total)
              : c.selectionCount(currentSelection.length)}
          </span>
          <button
            type="button"
            ref={confirmButton}
            className="button button--primary"
            disabled={working || !currentSelection.length}
            onClick={() => void approveSelected()}
          >
            {c.confirmSelected(currentSelection.length)}
          </button>
        </div>
      )}
      {selected.length !== currentSelection.length && (
        <p className="inline-alert" role="status">
          {c.selectionChanged}
        </p>
      )}
      {visible.map((review) => (
        <CompletionReviewCard
          key={review.id}
          review={review}
          busy={working}
          picked={selectedIds.has(review.id)}
          onPick={(checked) => {
            const otherSelections = currentSelection.filter(
              (item) => item.id !== review.id,
            );
            setSelected(
              checked ? [...otherSelections, review] : otherSelections,
            );
          }}
          saving={busy === review.id}
          initiallyOpen={highlightedId === review.id}
          onDecide={decide}
        />
      ))}
      {items.length > 3 && (
        <button
          className="button button--ghost"
          type="button"
          disabled={working}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? c.collapse : c.expand}
        </button>
      )}
      {expanded && cursor && (
        <button
          className="button button--ghost"
          type="button"
          disabled={loadingMore || working}
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
  picked = false,
  onPick,
}: {
  review: CompletionReview;
  busy: boolean;
  saving: boolean;
  initiallyOpen: boolean;
  picked?: boolean;
  onPick?(checked: boolean): void;
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
      <div className="completion-review__row">
        {onPick && (
          <label className="completion-reviews__pick">
            <ReviewCheckbox
              label={c.select(currentReview.title)}
              descriptionId={
                canConfirmTogether(currentReview)
                  ? undefined
                  : `review-changed-${review.id}`
              }
              checked={picked}
              disabled={busy || !canConfirmTogether(currentReview)}
              onChange={onPick}
            />
          </label>
        )}
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
            {onPick && !canConfirmTogether(currentReview) && (
              <small id={`review-changed-${review.id}`}>
                {c.needsIndividualReview}
              </small>
            )}
          </span>
          <ChevronDown size={18} aria-hidden="true" />
        </button>
      </div>
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
