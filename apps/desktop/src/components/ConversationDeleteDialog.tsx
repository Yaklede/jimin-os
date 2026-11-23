import { useEffect, useEffectEvent, useId, useRef, useState } from "react";

import { AgentRequestError } from "../api/agent";
import { conversationHistoryCopy as c } from "../copy/conversationHistory";
import { registerMobileBackHandler } from "../mobileBack";

export function ConversationDeleteDialog({
  title,
  count,
  onDelete,
  onClose,
}: {
  onDelete(): Promise<void>;
  onClose(deleted: boolean): void;
} & ({ title: string; count?: never } | { title?: never; count: number })) {
  const id = useId();
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const saving = useRef(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");

  function finish(deleted: boolean) {
    dialog.current?.close();
    onClose(deleted);
  }

  const handleBack = useEffectEvent(() => {
    if (!saving.current) finish(false);
    return true;
  });

  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    element.showModal();
    cancel.current?.focus({ preventScroll: true });
    const unregister = registerMobileBackHandler(() => handleBack(), 110);
    return () => {
      unregister();
      element.close();
    };
  }, []);

  async function submit() {
    if (saving.current) return;
    saving.current = true;
    setPending(true);
    setError("");
    try {
      await onDelete();
      if (dialog.current) finish(true);
    } catch (failure) {
      setError(
        failure instanceof AgentRequestError && failure.code === "conflict"
          ? c.deleteBusy
          : c.deleteProblem,
      );
    } finally {
      saving.current = false;
      setPending(false);
    }
  }

  return (
    <dialog
      ref={dialog}
      className="work-dialog conversation-delete-dialog"
      role="alertdialog"
      aria-labelledby={`${id}-title`}
      aria-describedby={`${id}-description`}
      aria-busy={pending}
      onCancel={(event) => {
        event.preventDefault();
        if (!saving.current) finish(false);
      }}
    >
      <form
        className="work-dialog__layout"
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <header className="work-dialog__header">
          <h2 id={`${id}-title`}>
            {count === undefined ? c.deleteTitle : c.bulkTitle(count)}
          </h2>
        </header>
        <div className="work-dialog__body">
          {title && <strong>{title}</strong>}
          <p id={`${id}-description`}>
            {count === undefined ? c.deleteDescription : c.bulkDescription}
          </p>
          {error && (
            <p className="inline-alert" role="alert">
              {error}
            </p>
          )}
        </div>
        <footer className="work-dialog__footer">
          <button
            type="button"
            ref={cancel}
            className="secondary-button focus-visible-control"
            disabled={pending}
            onClick={() => finish(false)}
          >
            {c.cancel}
          </button>
          <button
            type="submit"
            className="destructive-button focus-visible-control"
            disabled={pending}
          >
            {pending ? c.deleting : c.confirm}
          </button>
        </footer>
      </form>
    </dialog>
  );
}
