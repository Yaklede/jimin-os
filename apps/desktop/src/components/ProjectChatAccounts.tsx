import { useEffect, useEffectEvent, useId, useRef, useState } from "react";

import {
  GoogleChatRequestError,
  type GoogleChatAccount,
} from "../api/googleChat";
import { chatAccountsCopy as c } from "../copy/chatAccounts";
import { registerMobileBackHandler } from "../mobileBack";
import "./chatAccounts.css";

export function ProjectChatAccounts({
  accounts,
  available,
  busy,
  connectedHere,
  onDisconnect,
}: {
  accounts: readonly GoogleChatAccount[];
  available: boolean;
  busy: boolean;
  connectedHere: ReadonlyMap<string, number>;
  onDisconnect(account: GoogleChatAccount): Promise<void>;
}) {
  const id = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const opener = useRef<HTMLButtonElement | null>(null);
  const [target, setTarget] = useState<GoogleChatAccount>();
  const [statusMessage, setStatusMessage] = useState("");

  return (
    <section className="project-inflow__group" aria-labelledby={id}>
      <h4 id={id} ref={heading} tabIndex={-1}>
        {c.title}
      </h4>
      {!accounts.length && available && (
        <p className="project-inflow__empty">{c.empty}</p>
      )}
      {accounts.length > 0 && (
        <ul className="project-inflow__sources" aria-labelledby={id}>
          {accounts.map((account) => {
            const shown = connectedHere.get(account.id) ?? 0;
            const count = account.connectedSpaceCount;
            const used = Math.max(shown, count ?? 0);
            const canRemove = available && count === 0 && used === 0;
            return (
              <li key={account.id}>
                <div>
                  <strong title={account.email}>{account.email}</strong>
                  <span>
                    {used > 0
                      ? c.used(used)
                      : count === 0
                        ? c.unused
                        : c.unknown}
                  </span>
                  {account.reauthRequired && <span>{c.reconnect}</span>}
                </div>
                {canRemove && (
                  <button
                    type="button"
                    className="secondary-button focus-visible-control"
                    aria-label={c.removeLabel(account.email)}
                    disabled={busy}
                    onClick={(event) => {
                      opener.current = event.currentTarget;
                      setStatusMessage("");
                      setTarget(account);
                    }}
                  >
                    {c.remove}
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      )}
      <div role="status">{statusMessage}</div>
      {target && (
        <DisconnectAccountDialog
          account={target}
          onDisconnect={onDisconnect}
          onClose={(removed) => {
            if (removed) setStatusMessage(c.removed(target.email));
            setTarget(undefined);
            const returnTarget = removed ? heading.current : opener.current;
            (returnTarget?.isConnected ? returnTarget : heading.current)?.focus(
              { preventScroll: true },
            );
          }}
        />
      )}
    </section>
  );
}

function DisconnectAccountDialog({
  account,
  onDisconnect,
  onClose,
}: {
  account: GoogleChatAccount;
  onDisconnect(account: GoogleChatAccount): Promise<void>;
  onClose(removed: boolean): void;
}) {
  const id = useId();
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const running = useRef(false);
  const [pending, setPending] = useState(false);
  const [failure, setFailure] = useState<"changed" | "inUse" | "retry">();
  const blocked = failure === "changed" || failure === "inUse";

  function finish(removed: boolean) {
    dialog.current?.close();
    onClose(removed);
  }

  const handleBack = useEffectEvent(() => {
    if (!running.current) finish(false);
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

  useEffect(() => {
    if (blocked && !pending) cancel.current?.focus({ preventScroll: true });
  }, [blocked, pending]);

  async function submit() {
    if (running.current || blocked) return;
    running.current = true;
    setPending(true);
    setFailure(undefined);
    try {
      await onDisconnect(account);
      if (dialog.current) finish(true);
    } catch (error) {
      if (!dialog.current) return;
      if (
        error instanceof GoogleChatRequestError &&
        error.code === "conflict"
      ) {
        setFailure(
          error.serverCode === "google_chat.connection_in_use"
            ? "inUse"
            : "changed",
        );
      } else {
        setFailure("retry");
      }
    } finally {
      running.current = false;
      if (dialog.current) setPending(false);
    }
  }

  return (
    <dialog
      ref={dialog}
      className="work-dialog chat-account-disconnect"
      role="alertdialog"
      aria-labelledby={`${id}-title`}
      aria-describedby={`${id}-account ${id}-message`}
      aria-busy={pending}
      onCancel={(event) => {
        event.preventDefault();
        if (!running.current) finish(false);
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
          <h2 id={`${id}-title`}>{c.confirmTitle}</h2>
        </header>
        <div className="work-dialog__body">
          <strong id={`${id}-account`}>{account.email}</strong>
          <p id={`${id}-message`}>{c.confirmDescription}</p>
          {failure && (
            <p className="inline-alert" role="alert">
              {failure === "inUse"
                ? c.usedAfterOpening
                : failure === "changed"
                  ? c.changed
                  : c.removalFailed}
            </p>
          )}
        </div>
        <footer className="work-dialog__footer">
          <button
            ref={cancel}
            type="button"
            className="secondary-button focus-visible-control"
            disabled={pending}
            onClick={() => finish(false)}
          >
            {c.cancel}
          </button>
          <button
            type="submit"
            className="destructive-button focus-visible-control"
            disabled={pending || blocked}
          >
            {pending ? c.removing : c.remove}
          </button>
        </footer>
      </form>
    </dialog>
  );
}
