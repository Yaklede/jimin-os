import { useEffect, useId, useRef } from "react";

import { scheduledWorkCopy as c } from "../copy/scheduledWork";
import { registerMobileBackHandler } from "../mobileBack";

export function ScheduledWorkCloseConfirmation({
  open,
  onKeepEditing,
  onDiscard,
}: {
  open: boolean;
  onKeepEditing(): void;
  onDiscard(): void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const keepEditing = useRef<HTMLButtonElement>(null);
  const discard = useRef<HTMLButtonElement>(null);
  const descriptionId = useId();
  const keepEditingCallback = useRef(onKeepEditing);
  keepEditingCallback.current = onKeepEditing;

  useEffect(() => {
    if (!open) return;
    const element = dialog.current;
    if (!element) return;
    const previousFocus = document.activeElement;
    element.showModal();
    keepEditing.current?.focus({ preventScroll: true });
    const unregister = registerMobileBackHandler(() => {
      keepEditingCallback.current();
      return true;
    }, 110);
    return () => {
      unregister();
      element.close();
      if (
        previousFocus instanceof HTMLElement &&
        previousFocus.isConnected &&
        previousFocus.getClientRects().length > 0
      ) {
        previousFocus.focus({ preventScroll: true });
      }
    };
  }, [open]);

  if (!open) return null;
  return (
    <dialog
      ref={dialog}
      className="scheduled-discard-dialog"
      role="alertdialog"
      aria-label={c.discardTitle}
      aria-describedby={descriptionId}
      onKeyDown={(event) => {
        if (event.key !== "Tab") return;
        if (event.shiftKey && document.activeElement === keepEditing.current) {
          event.preventDefault();
          discard.current?.focus({ preventScroll: true });
        } else if (
          !event.shiftKey &&
          document.activeElement === discard.current
        ) {
          event.preventDefault();
          keepEditing.current?.focus({ preventScroll: true });
        }
      }}
      onCancel={(event) => {
        event.preventDefault();
        // React can propagate cancel to the enclosing reservation dialog.
        event.stopPropagation();
        onKeepEditing();
      }}
    >
      <h3>{c.discardTitle}</h3>
      <p id={descriptionId}>{c.closeHelp}</p>
      <div className="scheduled-actions">
        <button
          type="button"
          className="focus-visible-control"
          onClick={onKeepEditing}
          ref={keepEditing}
        >
          {c.keepEditing}
        </button>
        <button
          type="button"
          className="primary-button focus-visible-control"
          onClick={onDiscard}
          ref={discard}
        >
          {c.discard}
        </button>
      </div>
    </dialog>
  );
}
