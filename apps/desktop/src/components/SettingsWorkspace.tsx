import {
  CalendarDays,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Link2,
  LoaderCircle,
  RefreshCw,
} from "lucide-react";
import { useEffect, useState } from "react";

import {
  type AgentAuthentication,
  type AgentModelSettings,
} from "../api/agent";
import { type GoogleCalendarConnection } from "../api/calendar";
import { copy } from "../copy";

type SettingsWorkspaceProps = {
  authentication: AgentAuthentication | undefined;
  requesting: boolean;
  modelSettings: AgentModelSettings | undefined;
  modelsLoading: boolean;
  modelsSaving: boolean;
  modelsError: string | undefined;
  calendarConnection: GoogleCalendarConnection | undefined;
  calendarLoading: boolean;
  calendarAction: "authorizing" | "syncing" | undefined;
  calendarAuthorizationPending: boolean;
  calendarError: string | undefined;
  onStartAuthentication(): Promise<void>;
  onReloadModels(): Promise<void>;
  onSaveModel(
    modelId: string | null,
    reasoningEffort: string | null,
  ): Promise<boolean>;
  onStartCalendarConnection(): Promise<void>;
  onReloadCalendarConnection(): Promise<GoogleCalendarConnection | undefined>;
  onSyncCalendar(): Promise<void>;
};

export function SettingsWorkspace({
  authentication,
  requesting,
  modelSettings,
  modelsLoading,
  modelsSaving,
  modelsError,
  calendarConnection,
  calendarLoading,
  calendarAction,
  calendarAuthorizationPending,
  calendarError,
  onStartAuthentication,
  onReloadModels,
  onSaveModel,
  onStartCalendarConnection,
  onReloadCalendarConnection,
  onSyncCalendar,
}: SettingsWorkspaceProps) {
  const savedModelId = modelSettings?.selectedModelId ?? "";
  const savedReasoningEffort = modelSettings?.selectedReasoningEffort ?? "";
  const [draftModelId, setDraftModelId] = useState(savedModelId);
  const [draftReasoningEffort, setDraftReasoningEffort] =
    useState(savedReasoningEffort);
  const [modelSaved, setModelSaved] = useState(false);
  useEffect(() => {
    setDraftModelId(savedModelId);
    setDraftReasoningEffort(savedReasoningEffort);
  }, [savedModelId, savedReasoningEffort]);

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
  const defaultModel = modelSettings?.items.find((model) => model.isDefault);
  const draftModel =
    modelSettings?.items.find((model) => model.id === draftModelId) ??
    defaultModel;
  const waitingForModels = modelsLoading || (!modelSettings && !modelsError);
  const settingsChanged =
    draftModelId !== savedModelId ||
    draftReasoningEffort !== savedReasoningEffort;
  const calendarReady = calendarConnection?.status === "active";
  const calendarUnavailable = calendarConnection?.available === false;
  const calendarNeedsAttention =
    calendarConnection?.status === "reauth_required" ||
    calendarConnection?.status === "revoked" ||
    calendarConnection?.status === "error";
  const calendarBusy = calendarLoading || Boolean(calendarAction);
  const calendarDetail = calendarConnectionDetail(
    calendarConnection,
    calendarLoading,
    calendarAuthorizationPending,
  );

  async function saveModel() {
    setModelSaved(false);
    if (await onSaveModel(draftModelId || null, draftReasoningEffort || null)) {
      setModelSaved(true);
    }
  }

  return (
    <section className="settings-page">
      <header className="page-heading">
        <p>개인 워크스페이스</p>
        <h1>{copy.settings.title}</h1>
        <span>{copy.settings.description}</span>
      </header>
      <section className="settings-list" aria-label={copy.settings.title}>
        <div className="settings-model-field">
          <div className="settings-model-field__heading">
            <strong>{copy.settings.modelTitle}</strong>
            <p id="processing-model-description">
              {copy.settings.modelDescription}
            </p>
          </div>
          <div className="settings-model-field__control">
            <div className="settings-model-field__option">
              <label htmlFor="processing-model">
                {copy.settings.modelFieldLabel}
              </label>
              <select
                id="processing-model"
                className="focus-visible-control"
                value={draftModelId}
                aria-describedby="processing-model-description processing-model-feedback"
                disabled={waitingForModels || modelsSaving}
                onChange={(event) => {
                  const nextModelId = event.target.value;
                  const nextModel =
                    modelSettings?.items.find(
                      (model) => model.id === nextModelId,
                    ) ?? defaultModel;
                  setDraftModelId(nextModelId);
                  if (
                    draftReasoningEffort &&
                    !nextModel?.supportedReasoningEfforts.some(
                      (effort) => effort.id === draftReasoningEffort,
                    )
                  ) {
                    setDraftReasoningEffort("");
                  }
                  setModelSaved(false);
                }}
              >
                <option value="">
                  {copy.settings.modelAutomatic(defaultModel?.displayName)}
                </option>
                {modelSettings?.items.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.displayName}
                  </option>
                ))}
              </select>
            </div>
            <div className="settings-model-field__option">
              <label htmlFor="reasoning-effort">
                {copy.settings.effortTitle}
              </label>
              <select
                id="reasoning-effort"
                className="focus-visible-control"
                value={draftReasoningEffort}
                aria-describedby="processing-model-description processing-model-feedback"
                disabled={
                  waitingForModels ||
                  modelsSaving ||
                  !draftModel?.supportedReasoningEfforts.length
                }
                onChange={(event) => {
                  setDraftReasoningEffort(event.target.value);
                  setModelSaved(false);
                }}
              >
                <option value="">
                  {copy.settings.effortAutomatic(
                    draftModel?.defaultReasoningEffort,
                  )}
                </option>
                {draftModel?.supportedReasoningEfforts.map((effort) => (
                  <option key={effort.id} value={effort.id}>
                    {copy.settings.effortLabel(effort.id)}
                  </option>
                ))}
              </select>
            </div>
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={waitingForModels || modelsSaving || !settingsChanged}
              onClick={() => void saveModel()}
            >
              {modelsSaving ? (
                <LoaderCircle className="spin" aria-hidden="true" />
              ) : null}
              {modelsSaving
                ? copy.settings.modelSaving
                : copy.settings.modelSave}
            </button>
          </div>
          <div
            id="processing-model-feedback"
            className="settings-model-field__feedback"
            aria-live="polite"
          >
            {waitingForModels ? (
              <span>{copy.settings.modelLoading}</span>
            ) : modelsError ? (
              <span className="settings-model-field__error" role="alert">
                {modelsError}
                <button
                  className="text-button focus-visible-control"
                  type="button"
                  disabled={modelsLoading}
                  onClick={() => void onReloadModels()}
                >
                  {copy.settings.modelReload}
                </button>
              </span>
            ) : modelSettings?.items.length === 0 ? (
              <span>{copy.settings.modelEmpty}</span>
            ) : modelSaved ? (
              <span className="settings-model-field__success">
                <CheckCircle2 aria-hidden="true" />
                {copy.settings.modelSaved}
              </span>
            ) : (
              <span>
                {copy.settings.modelCurrent(
                  modelSettings?.items.find(
                    (model) => model.id === savedModelId,
                  )?.displayName ?? defaultModel?.displayName,
                  copy.settings.effortLabel(
                    modelSettings?.selectedReasoningEffort ??
                      (
                        modelSettings?.items.find(
                          (model) => model.id === savedModelId,
                        ) ?? defaultModel
                      )?.defaultReasoningEffort,
                  ),
                )}
              </span>
            )}
          </div>
        </div>
        <div className="settings-list__section-heading">
          <strong>{copy.settings.connectionsTitle}</strong>
          <p>{copy.settings.connectionsDescription}</p>
        </div>
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
        <div
          className="settings-row"
          data-state={
            calendarUnavailable ? "unavailable" : calendarConnection?.status
          }
        >
          <span className="settings-row__icon" aria-hidden="true">
            {calendarBusy ? (
              <LoaderCircle className="spin" />
            ) : calendarReady ? (
              <CheckCircle2 />
            ) : calendarUnavailable || calendarNeedsAttention ? (
              <CircleAlert />
            ) : (
              <CalendarDays />
            )}
          </span>
          <div className="settings-row__copy">
            <strong>{copy.settings.calendarTitle}</strong>
            <p>{calendarDetail}</p>
            {calendarError && (
              <p className="settings-row__error" role="alert">
                {calendarError}
              </p>
            )}
          </div>
          <div className="settings-row__actions">
            {calendarUnavailable ? (
              <span className="settings-row__state settings-row__state--warning">
                {copy.settings.calendarConfigurationRequired}
              </span>
            ) : !calendarConnection ? (
              <button
                className="text-button focus-visible-control"
                type="button"
                disabled={calendarLoading}
                onClick={() => void onReloadCalendarConnection()}
              >
                {calendarLoading ? (
                  <LoaderCircle className="spin" aria-hidden="true" />
                ) : (
                  <RefreshCw aria-hidden="true" />
                )}
                {calendarLoading
                  ? copy.settings.calendarChecking
                  : copy.settings.calendarRetry}
              </button>
            ) : calendarReady ? (
              <button
                className="text-button focus-visible-control"
                type="button"
                disabled={calendarBusy}
                onClick={() => void onSyncCalendar()}
              >
                {calendarAction === "syncing" ? (
                  <LoaderCircle className="spin" aria-hidden="true" />
                ) : (
                  <RefreshCw aria-hidden="true" />
                )}
                {calendarAction === "syncing"
                  ? copy.settings.calendarSyncing
                  : copy.settings.calendarSync}
              </button>
            ) : calendarAuthorizationPending ? (
              <button
                className="text-button focus-visible-control"
                type="button"
                disabled={calendarBusy}
                onClick={() => void onReloadCalendarConnection()}
              >
                {calendarLoading ? (
                  <LoaderCircle className="spin" aria-hidden="true" />
                ) : null}
                {calendarLoading
                  ? copy.settings.calendarChecking
                  : copy.settings.calendarCheckConnection}
              </button>
            ) : (
              <button
                className="text-button focus-visible-control"
                type="button"
                disabled={calendarBusy}
                onClick={() => void onStartCalendarConnection()}
              >
                {calendarAction === "authorizing" ? (
                  <LoaderCircle className="spin" aria-hidden="true" />
                ) : null}
                {calendarAction === "authorizing"
                  ? copy.settings.calendarOpening
                  : calendarNeedsAttention
                    ? copy.settings.calendarReconnect
                    : copy.settings.calendarConnect}
                {!calendarBusy && <ChevronRight aria-hidden="true" />}
              </button>
            )}
          </div>
        </div>
      </section>
    </section>
  );
}

function calendarConnectionDetail(
  connection: GoogleCalendarConnection | undefined,
  loading: boolean,
  authorizationPending: boolean,
): string {
  if (!connection && loading) return copy.settings.calendarLoading;
  if (!connection) return copy.settings.calendarLoadFailed;
  if (!connection.available) return copy.settings.calendarConfigurationMissing;
  if (authorizationPending && connection.status !== "active") {
    return copy.settings.calendarAwaitingAuthorization;
  }
  if (connection.status === "active") {
    return copy.settings.calendarConnected(
      connection.email ?? undefined,
      connection.lastSuccessfulSyncAt ?? undefined,
    );
  }
  if (connection.status === "reauth_required") {
    return copy.settings.calendarReauthRequired;
  }
  if (connection.status === "connecting") {
    return copy.settings.calendarAwaitingAuthorization;
  }
  if (connection.status === "revoking") {
    return copy.settings.calendarDisconnecting;
  }
  if (connection.status === "revoked" || connection.status === "error") {
    return copy.settings.calendarNeedsReconnect;
  }
  return copy.settings.calendarNotConnected;
}
