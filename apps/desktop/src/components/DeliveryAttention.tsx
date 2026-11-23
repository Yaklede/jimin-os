import { AlertTriangle, ChevronRight, X } from "lucide-react";
import { createPortal } from "react-dom";
import { deliveryAttentionCopy as c } from "../copy/deliveryAttention";

export type DeliveryNotice = { message: string; failed: boolean };

export function DeliveryAttention({
  count,
  onOpen,
}: {
  count: number;
  onOpen(): void;
}) {
  if (!count) return null;
  return (
    <button
      type="button"
      className="home-delivery-attention focus-visible-control"
      onClick={onOpen}
    >
      <AlertTriangle aria-hidden="true" />
      <strong>{c.count(count)}</strong>
      <ChevronRight aria-hidden="true" />
    </button>
  );
}

export function DeliveryFeedback({
  notice,
  onOpen,
  onClose,
}: {
  notice?: DeliveryNotice;
  onOpen(): void;
  onClose(): void;
}) {
  if (!notice || typeof document === "undefined") return null;
  if (!notice.failed)
    return createPortal(
      <div className="home-inflow-review__receipt" role="status">
        <p>{notice.message}</p>
      </div>,
      document.body,
    );
  return createPortal(
    <div className="delivery-feedback">
      <p role="alert">{notice.message}</p>
      <button
        type="button"
        className="text-button focus-visible-control"
        onClick={onOpen}
      >
        {c.openHistory}
      </button>
      <button
        type="button"
        className="delivery-feedback__close focus-visible-control"
        aria-label={c.close}
        onClick={onClose}
      >
        <X aria-hidden="true" />
      </button>
    </div>,
    document.body,
  );
}
