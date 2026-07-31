import {
  CheckCircle2,
  Database,
  LoaderCircle,
  RefreshCw,
  Unlink,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import {
  type ProjectItsmConnection,
  type ProjectItsmConnectionSnapshot,
} from "../api/itsm";
import { copy } from "../copy";

type PendingAction =
  "connecting" | "confirming" | "reloading" | "disconnecting";

type ProjectItsmConnectionPanelProps = {
  snapshot: ProjectItsmConnectionSnapshot | undefined;
  loading: boolean;
  saving: boolean;
  problemMessage?: string;
  onReload(): Promise<void>;
  onConnect(): Promise<void>;
  onConfirm(connection: ProjectItsmConnection): Promise<void>;
  onDisconnect(connection: ProjectItsmConnection): Promise<void>;
};

export function ProjectItsmConnectionPanel({
  snapshot,
  loading,
  saving,
  problemMessage,
  onReload,
  onConnect,
  onConfirm,
  onDisconnect,
}: ProjectItsmConnectionPanelProps) {
  const [pendingAction, setPendingAction] = useState<PendingAction>();
  const [disconnectOpen, setDisconnectOpen] = useState(false);
  const [notice, setNotice] = useState<string>();
  const [localProblem, setLocalProblem] = useState<string>();
  const disconnectTriggerRef = useRef<HTMLButtonElement>(null);
  const disconnectSafeActionRef = useRef<HTMLButtonElement>(null);
  const restoreDisconnectFocusRef = useRef(false);
  const connection = snapshot?.item;
  const enabled = Boolean(connection?.enabled);
  const connected = connection?.confirmationStatus === "confirmed";
  const confirmationRequired =
    connection?.confirmationStatus === "confirmation_required";
  const discovering = connection?.confirmationStatus === "discovering";
  const busy = loading || saving || pendingAction !== undefined;
  const problem = localProblem ?? problemMessage;

  useEffect(() => {
    setDisconnectOpen(false);
    setNotice(undefined);
    setLocalProblem(undefined);
    setPendingAction(undefined);
    restoreDisconnectFocusRef.current = false;
  }, [connection?.projectId]);

  useEffect(() => {
    const target = disconnectOpen
      ? disconnectSafeActionRef.current
      : restoreDisconnectFocusRef.current
        ? disconnectTriggerRef.current
        : undefined;
    if (!target) return;
    const frame = window.requestAnimationFrame(() => {
      target.focus();
      if (!disconnectOpen) restoreDisconnectFocusRef.current = false;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [disconnectOpen]);

  async function reload() {
    if (busy) return;
    setPendingAction("reloading");
    setLocalProblem(undefined);
    setNotice(undefined);
    try {
      await onReload();
    } catch {
      setLocalProblem(copy.projects.itsmLoadProblem);
    } finally {
      setPendingAction(undefined);
    }
  }

  async function connect() {
    if (busy) return;
    setPendingAction("connecting");
    setLocalProblem(undefined);
    setNotice(undefined);
    try {
      await onConnect();
      setNotice(copy.projects.itsmConnectedNotice);
    } catch {
      setLocalProblem(copy.projects.itsmConnectProblem);
    } finally {
      setPendingAction(undefined);
    }
  }

  async function disconnect() {
    if (!connection || busy) return;
    setPendingAction("disconnecting");
    setLocalProblem(undefined);
    setNotice(undefined);
    try {
      await onDisconnect(connection);
      setDisconnectOpen(false);
      setNotice(copy.projects.itsmDisconnectedNotice);
    } catch {
      setLocalProblem(copy.projects.itsmDisconnectProblem);
    } finally {
      setPendingAction(undefined);
    }
  }

  async function confirm() {
    if (!connection || busy || !confirmationRequired) return;
    setPendingAction("confirming");
    setLocalProblem(undefined);
    setNotice(undefined);
    try {
      await onConfirm(connection);
      setNotice(copy.projects.itsmConfirmedNotice);
    } catch {
      setLocalProblem(copy.projects.itsmConfirmProblem);
    } finally {
      setPendingAction(undefined);
    }
  }

  return (
    <section
      className="project-itsm"
      aria-labelledby="project-itsm-title"
      aria-busy={busy}
    >
      <div className="project-itsm__main">
        <span className="project-itsm__icon" aria-hidden="true">
          {loading || pendingAction === "reloading" ? (
            <LoaderCircle className="spin" />
          ) : connected ? (
            <CheckCircle2 />
          ) : (
            <Database />
          )}
        </span>
        <div className="project-itsm__copy">
          <h4 id="project-itsm-title">{copy.projects.itsmTitle}</h4>
          <p>{copy.projects.itsmDescription}</p>
          {!loading && snapshot?.available && (
            <span>
              {connected
                ? copy.projects.itsmConnectedDescription
                : confirmationRequired
                  ? copy.projects.itsmConfirmationDescription
                  : discovering
                    ? copy.projects.itsmDiscoveringDescription
                    : copy.projects.itsmAvailableDescription}
            </span>
          )}
        </div>
        {enabled && (
          <span
            className="project-itsm__status"
            data-status={connection?.confirmationStatus}
          >
            {connected
              ? copy.projects.itsmConnected
              : confirmationRequired
                ? copy.projects.itsmConfirmationRequired
                : copy.projects.itsmDiscovering}
          </span>
        )}
      </div>

      {problem && (
        <p className="inline-alert" role="alert">
          {problem}
        </p>
      )}
      {notice && !problem && (
        <p className="project-save-status" role="status">
          {notice}
        </p>
      )}

      {loading && snapshot === undefined ? (
        <p className="project-itsm__loading" role="status">
          <LoaderCircle className="spin" aria-hidden="true" />
          {copy.projects.itsmLoading}
        </p>
      ) : snapshot?.available === false ? (
        <div className="project-itsm__unavailable">
          <div>
            <strong>{copy.projects.itsmNeedsSetupTitle}</strong>
            <p>{copy.projects.itsmNeedsSetupDescription}</p>
          </div>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={busy}
            onClick={() => void reload()}
          >
            {pendingAction === "reloading" ? (
              <LoaderCircle className="spin" aria-hidden="true" />
            ) : (
              <RefreshCw aria-hidden="true" />
            )}
            {pendingAction === "reloading"
              ? copy.projects.itsmReloading
              : copy.projects.itsmReload}
          </button>
        </div>
      ) : enabled && connection ? (
        <>
          {confirmationRequired && connection.candidateProjectName && (
            <div className="project-itsm__confirmation">
              <div>
                <strong>
                  {copy.projects.itsmCandidateTitle(
                    connection.candidateProjectName,
                  )}
                </strong>
                <p>{copy.projects.itsmCandidateDescription}</p>
              </div>
              <button
                className="primary-button focus-visible-control"
                type="button"
                disabled={busy}
                onClick={() => void confirm()}
              >
                {pendingAction === "confirming" ? (
                  <LoaderCircle className="spin" aria-hidden="true" />
                ) : (
                  <CheckCircle2 aria-hidden="true" />
                )}
                {pendingAction === "confirming"
                  ? copy.projects.itsmConfirming
                  : copy.projects.itsmConfirm}
              </button>
            </div>
          )}
          {discovering && (
            <p className="project-itsm__discovering" role="status">
              <LoaderCircle className="spin" aria-hidden="true" />
              {copy.projects.itsmDiscoveringHelp}
            </p>
          )}
          <div className="project-itsm__actions">
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={busy}
              onClick={() => void reload()}
            >
              {pendingAction === "reloading" ? (
                <LoaderCircle className="spin" aria-hidden="true" />
              ) : (
                <RefreshCw aria-hidden="true" />
              )}
              {pendingAction === "reloading"
                ? copy.projects.itsmReloading
                : copy.projects.itsmReload}
            </button>
            {!disconnectOpen && (
              <button
                ref={disconnectTriggerRef}
                className="text-button text-button--danger focus-visible-control"
                type="button"
                disabled={busy}
                aria-expanded={disconnectOpen}
                onClick={() => setDisconnectOpen(true)}
              >
                <Unlink aria-hidden="true" />
                {copy.projects.itsmDisconnect}
              </button>
            )}
          </div>
          {disconnectOpen && (
            <div
              className="project-itsm__disconnect-confirmation"
              role="group"
              aria-label={copy.projects.itsmDisconnect}
            >
              <p>{copy.projects.itsmDisconnectConfirm}</p>
              <div>
                <button
                  ref={disconnectSafeActionRef}
                  className="secondary-button focus-visible-control"
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    restoreDisconnectFocusRef.current = true;
                    setDisconnectOpen(false);
                  }}
                >
                  {copy.projects.itsmKeep}
                </button>
                <button
                  className="destructive-button focus-visible-control"
                  type="button"
                  disabled={busy}
                  onClick={() => void disconnect()}
                >
                  {pendingAction === "disconnecting" ? (
                    <LoaderCircle className="spin" aria-hidden="true" />
                  ) : (
                    <Unlink aria-hidden="true" />
                  )}
                  {pendingAction === "disconnecting"
                    ? copy.projects.itsmDisconnecting
                    : copy.projects.itsmDisconnectAction}
                </button>
              </div>
            </div>
          )}
        </>
      ) : snapshot?.available ? (
        <div className="project-itsm__actions project-itsm__actions--connect">
          <button
            className="primary-button focus-visible-control"
            type="button"
            disabled={busy}
            onClick={() => void connect()}
          >
            {pendingAction === "connecting" ? (
              <LoaderCircle className="spin" aria-hidden="true" />
            ) : (
              <Database aria-hidden="true" />
            )}
            {pendingAction === "connecting"
              ? copy.projects.itsmConnecting
              : copy.projects.itsmConnect}
          </button>
        </div>
      ) : problem ? (
        <div className="project-itsm__actions project-itsm__actions--connect">
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={busy}
            onClick={() => void reload()}
          >
            <RefreshCw aria-hidden="true" />
            {copy.projects.itsmReload}
          </button>
        </div>
      ) : null}
    </section>
  );
}
