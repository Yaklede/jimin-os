import {
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Link2,
  LoaderCircle,
} from "lucide-react";

import { type AgentAuthentication } from "../api/agent";
import { copy } from "../copy";

type SettingsWorkspaceProps = {
  authentication: AgentAuthentication | undefined;
  requesting: boolean;
  onStartAuthentication(): Promise<void>;
};

export function SettingsWorkspace({
  authentication,
  requesting,
  onStartAuthentication,
}: SettingsWorkspaceProps) {
  const state = authentication?.state ?? "requested";
  const ready = state === "ready";
  const waiting = state === "requested" || state === "awaiting_authorization";
  const failed = state === "failed";
  const detail = ready
    ? copy.settings.chatgptReady
    : state === "awaiting_authorization"
      ? copy.settings.chatgptAwaiting
      : failed
        ? copy.settings.chatgptFailed
        : waiting
          ? copy.settings.chatgptPreparing
          : copy.settings.chatgptNeedsLogin;

  return (
    <section className="settings-page">
      <header className="page-heading">
        <p>개인 워크스페이스</p>
        <h1>{copy.settings.title}</h1>
        <span>{copy.settings.description}</span>
      </header>
      <section className="settings-list" aria-label={copy.settings.title}>
        <div className="settings-row">
          <span className="settings-row__icon" aria-hidden="true">
            {ready ? (
              <CheckCircle2 />
            ) : failed ? (
              <CircleAlert />
            ) : waiting ? (
              <LoaderCircle className="spin" />
            ) : (
              <Link2 />
            )}
          </span>
          <div>
            <strong>{copy.settings.chatgptTitle}</strong>
            <p>{detail}</p>
          </div>
          {ready ? (
            <span className="settings-row__state">
              {copy.settings.chatgptReady}
            </span>
          ) : (
            <button
              className="text-button focus-visible-control"
              type="button"
              onClick={() => void onStartAuthentication()}
              disabled={requesting || waiting}
            >
              {failed
                ? copy.actions.retryChatgptConnection
                : copy.actions.connectChatgpt}
              <ChevronRight aria-hidden="true" />
            </button>
          )}
        </div>
      </section>
    </section>
  );
}
