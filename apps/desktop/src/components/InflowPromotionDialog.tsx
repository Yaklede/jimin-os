import { ListTodo, X } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";

import { registerMobileBackHandler } from "../mobileBack";

export const inflowPromotionDialogCopy = {
  title: "할 일로 등록하기",
  description: "내용과 담당자, 마감일을 확인하고 등록해 주세요.",
  close: "등록 화면 닫기",
  discardTitle: "작성한 내용을 버릴까요?",
  discardDescription: "아직 등록하지 않았어요. 닫으면 수정한 내용이 사라져요.",
  keepEditing: "계속 작성하기",
  discard: "버리고 닫기",
  registerAndNotify: "등록하고 알리기",
};

export function inflowPromotionCloseAction(busy: boolean, dirty: boolean) {
  return busy ? "blocked" : dirty ? "confirm" : "close";
}

export function InflowPromotionDialog({
  open,
  busy,
  dirty,
  onClose,
  children,
}: {
  open: boolean;
  busy: boolean;
  dirty: boolean;
  onClose(): void;
  children: ReactNode | ((requestClose: () => void) => ReactNode);
}) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const keepEditingRef = useRef<HTMLButtonElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const [confirming, setConfirming] = useState(false);
  const closeRef = useRef(() => {});

  function requestClose() {
    const action = inflowPromotionCloseAction(busy, dirty);
    if (action === "blocked") return;
    if (action === "confirm") {
      previousFocusRef.current =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      setConfirming(true);
      return;
    }
    onClose();
  }
  closeRef.current = () => {
    if (confirming && !busy) setConfirming(false);
    else requestClose();
  };

  useEffect(() => {
    if (!open) return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const opener = document.activeElement;
    const rootOverflow = document.documentElement.style.overflow;
    const bodyOverflow = document.body.style.overflow;
    document.documentElement.style.overflow = "hidden";
    document.body.style.overflow = "hidden";
    setConfirming(false);
    dialog.showModal();
    const frame = window.requestAnimationFrame(() => {
      dialog.querySelector<HTMLInputElement>("form input")?.focus();
    });
    const unregister = registerMobileBackHandler(() => {
      closeRef.current();
      return true;
    }, 110);
    return () => {
      unregister();
      window.cancelAnimationFrame(frame);
      dialog.close();
      document.documentElement.style.overflow = rootOverflow;
      document.body.style.overflow = bodyOverflow;
      if (opener instanceof HTMLElement && opener.isConnected) {
        opener.focus({ preventScroll: true });
      }
    };
  }, [open]);

  useEffect(() => {
    if (confirming) keepEditingRef.current?.focus();
    else if (previousFocusRef.current?.isConnected) {
      previousFocusRef.current.focus();
    }
  }, [confirming]);

  if (!open) return null;
  return (
    <dialog
      ref={dialogRef}
      className="inflow-promotion-dialog"
      aria-label={inflowPromotionDialogCopy.title}
      aria-busy={busy}
      onKeyDown={(event) => {
        if (event.key !== "Tab") return;
        const controls = Array.from(
          event.currentTarget.querySelectorAll<HTMLElement>(
            'button:not(:disabled), input:not(:disabled):not([type="hidden"]), textarea:not(:disabled), select:not(:disabled), summary, a[href], [tabindex]:not([tabindex="-1"])',
          ),
        ).filter((control) => control.getClientRects().length > 0);
        const first = controls[0];
        const last = controls[controls.length - 1];
        if (!first || !last) return;
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }}
      onCancel={(event) => {
        event.preventDefault();
        if (confirming) {
          setConfirming(false);
        } else requestClose();
      }}
      onClick={(event) => {
        if (event.target !== event.currentTarget) return;
        const bounds = event.currentTarget.getBoundingClientRect();
        if (
          event.clientX < bounds.left ||
          event.clientX > bounds.right ||
          event.clientY < bounds.top ||
          event.clientY > bounds.bottom
        )
          requestClose();
      }}
    >
      <header className="inflow-promotion-dialog__heading">
        <ListTodo aria-hidden="true" />
        <div>
          <h2>{inflowPromotionDialogCopy.title}</h2>
          <p>{inflowPromotionDialogCopy.description}</p>
        </div>
        <button
          type="button"
          className="inflow-promotion-dialog__close focus-visible-control"
          aria-label={inflowPromotionDialogCopy.close}
          disabled={busy}
          onClick={requestClose}
        >
          <X aria-hidden="true" />
        </button>
      </header>
      <div className="inflow-promotion-dialog__content" hidden={confirming}>
        {typeof children === "function" ? children(requestClose) : children}
      </div>
      {confirming && (
        <section
          className="inflow-promotion-dialog__confirmation"
          role="alertdialog"
          aria-label={inflowPromotionDialogCopy.discardTitle}
        >
          <h3>{inflowPromotionDialogCopy.discardTitle}</h3>
          <p>{inflowPromotionDialogCopy.discardDescription}</p>
          <div>
            <button
              ref={keepEditingRef}
              type="button"
              className="primary-button focus-visible-control"
              onClick={() => setConfirming(false)}
            >
              {inflowPromotionDialogCopy.keepEditing}
            </button>
            <button
              type="button"
              className="secondary-button focus-visible-control"
              disabled={busy}
              onClick={onClose}
            >
              {inflowPromotionDialogCopy.discard}
            </button>
          </div>
        </section>
      )}
    </dialog>
  );
}
